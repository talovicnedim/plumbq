import { toast } from 'svelte-sonner';
import { api, toApiError, type HistoryEntry, type IdChange, type Request } from '#lib/api/index.js';
import { confirm } from './confirm.svelte.js';
import { recentRequests } from './recent-requests.svelte.js';
import { sendTab } from './tab-send.svelte.js';
import { idAfterChanges, isInFolder } from '#lib/utils/tab-ids.js';

export interface RequestTab {
	id: string;
	/** As on disk. */
	saved: Request;
	/** As in the editor. */
	draft: Request;
	/** Last response shown, from this session or picked from history. */
	entry?: HistoryEntry;
	sending: boolean;
	/** Why the last send failed (connection refused, unknown variable, …), shown in the response panel. */
	error?: string;
	/** `Date.now()` when the current send started, for the live timer. */
	startedAt?: number;
	/** The last send was cancelled by the user; shown as "Cancelled", not as an error. */
	cancelled?: boolean;
}

/** Open requests. A tab keeps unsaved edits until saved or closed. */
class TabsState {
	all = $state<RequestTab[]>([]);
	activeId = $state<string | null>(null);

	readonly active: RequestTab | undefined = $derived(
		this.all.find((tab) => tab.id === this.activeId)
	);

	async open(id: string): Promise<void> {
		if (!this.all.some((tab) => tab.id === id)) {
			try {
				const request = await api.requests.get(id);
				this.all.push({ id, saved: request, draft: clone(request), sending: false });
			} catch (err) {
				toast.error(toApiError(err).message);
				return;
			}
		}
		this.activeId = id;
		recentRequests.opened(id);
	}

	isDirty(tab: RequestTab): boolean {
		return JSON.stringify(tab.saved) !== JSON.stringify(tab.draft);
	}

	/** Closes a tab, asking first when it has unsaved changes. */
	async close(id: string): Promise<void> {
		const tab = this.all.find((t) => t.id === id);
		if (!tab) return;
		if (this.isDirty(tab)) {
			const discard = await confirm({
				title: 'Discard changes?',
				description: `"${tab.draft.name || tab.id}" has unsaved changes.`,
				confirmLabel: 'Discard',
				destructive: true
			});
			if (!discard) return;
		}
		this.drop(id);
	}

	/** Closes a tab without asking; for callers that already asked about its edits. */
	drop(id: string): void {
		const index = this.all.findIndex((t) => t.id === id);
		if (index === -1) return;
		this.all.splice(index, 1);
		if (this.activeId === id)
			this.activeId = this.all[Math.min(index, this.all.length - 1)]?.id ?? null;
	}

	/** Closes every tab without asking; for callers that already asked about edits. */
	dropAll(): void {
		this.all = [];
		this.activeId = null;
	}

	/** Whether any open tab has unsaved changes. */
	get anyDirty(): boolean {
		return this.all.some((t) => this.isDirty(t));
	}

	/** Closes several tabs, asking once if any has unsaved changes. Returns whether they closed. */
	async closeMany(ids: string[]): Promise<boolean> {
		const closing = this.all.filter((t) => ids.includes(t.id));
		const dirty = closing.filter((t) => this.isDirty(t));
		if (dirty.length > 0) {
			const discard = await confirm({
				title: 'Discard changes?',
				description:
					dirty.length === 1
						? `"${dirty[0].draft.name || dirty[0].id}" has unsaved changes.`
						: `${dirty.length} requests have unsaved changes.`,
				confirmLabel: 'Discard',
				destructive: true
			});
			if (!discard) return false;
		}
		const activeIndex = this.all.findIndex((t) => t.id === this.activeId);
		this.all = this.all.filter((t) => !ids.includes(t.id));
		if (this.activeId && ids.includes(this.activeId)) {
			this.activeId = this.all[Math.min(activeIndex, this.all.length - 1)]?.id ?? null;
		}
		return true;
	}

	closeOthers(id: string): Promise<boolean> {
		return this.closeMany(this.all.filter((t) => t.id !== id).map((t) => t.id));
	}

	closeToRight(id: string): Promise<boolean> {
		const index = this.all.findIndex((t) => t.id === id);
		return this.closeMany(this.all.slice(index + 1).map((t) => t.id));
	}

	closeEverything(): Promise<boolean> {
		return this.closeMany(this.all.map((t) => t.id));
	}

	/** Moves a tab to `index` (drag and drop in the tab bar). */
	move(id: string, index: number): void {
		const from = this.all.findIndex((t) => t.id === id);
		if (from === -1 || from === index) return;
		const [tab] = this.all.splice(from, 1);
		this.all.splice(Math.min(index, this.all.length), 0, tab);
	}

	async save(tab: RequestTab): Promise<void> {
		// What is written is what counts as saved; keystrokes during the write stay unsaved.
		const written = clone(tab.draft);
		try {
			// The source the tab read: core refuses the write if the file changed since.
			const saved = await api.requests.update(tab.id, written, tab.saved.source ?? null);
			tab.saved = { ...written, source: saved.source };
			tab.draft.source = saved.source;
		} catch (err) {
			const error = toApiError(err);
			if (error.kind === 'conflict') this.#offerConflictChoices(tab);
			else toast.error(error.message);
		}
	}

	/**
	 * The file changed on disk since the tab read it (an editor, git or an agent). The draft is
	 * kept; the user picks: take the disk version, or keep theirs as a new request.
	 */
	#offerConflictChoices(tab: RequestTab): void {
		const id = tab.id;
		// One toast per tab: auto-save hits the conflict again after every pause in typing.
		toast.warning(`"${tab.draft.name || id}" changed on disk since you opened it`, {
			id: `conflict:${id}`,
			description: 'Your edits are kept. Reload the saved version, or save yours as a new request.',
			duration: Infinity,
			action: { label: 'Save as new request', onClick: () => void this.#saveAsNew(id) },
			cancel: { label: 'Reload from disk', onClick: () => void this.#reloadFromDisk(id) }
		});
	}

	async #reloadFromDisk(id: string): Promise<void> {
		const tab = this.all.find((t) => t.id === id);
		if (!tab) return;
		try {
			const fresh = await api.requests.get(id);
			tab.saved = fresh;
			tab.draft = clone(fresh);
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async #saveAsNew(id: string): Promise<void> {
		const tab = this.all.find((t) => t.id === id);
		if (!tab) return;
		const folder = id.includes('/') ? id.slice(0, id.lastIndexOf('/')) : '';
		try {
			const newId = await api.requests.create(folder, { ...clone(tab.draft), source: '' });
			// The edits now live in the new request; the old tab goes back to what's on disk.
			tab.draft = clone(tab.saved);
			await this.open(newId);
			void this.reload();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	/** Sends the draft. Writes to production ask first; core enforces it regardless. */
	send(tab: RequestTab): Promise<void> {
		return sendTab(tab);
	}

	/** Stops the tab's in-flight send; the send then ends as "Cancelled". */
	async cancel(tab: RequestTab): Promise<void> {
		if (tab.sending) await api.requests.cancel(tab.id);
	}

	/**
	 * Follows the id changes core reported for a move, removal or folder rename, so tabs (and
	 * their unsaved edits) keep pointing at the same request. Tabs of removed requests keep
	 * their id; callers close them.
	 */
	applyChanges(changes: IdChange[]): void {
		this.#remap((id) => idAfterChanges(id, changes) ?? id);
		recentRequests.follow(changes);
	}

	/** Closes the tabs of requests inside a folder about to be deleted; `false` if the user kept them. */
	closeFolder(path: string): Promise<boolean> {
		return this.closeMany(this.all.filter((t) => isInFolder(t.id, path)).map((t) => t.id));
	}

	#remap(next: (id: string) => string): void {
		for (const tab of this.all) tab.id = next(tab.id);
		if (this.activeId) this.activeId = next(this.activeId);
	}

	#reloading: Promise<void> | null = null;
	#reloadAgain = false;

	/**
	 * Re-reads tabs without unsaved edits after files changed on disk; drops deleted ones. Runs
	 * one at a time: a change arriving mid-reload triggers one more pass afterwards.
	 */
	reload(): Promise<void> {
		if (this.#reloading) {
			this.#reloadAgain = true;
			return this.#reloading;
		}
		this.#reloading = (async () => {
			do {
				this.#reloadAgain = false;
				await this.#reloadOnce();
			} while (this.#reloadAgain);
			this.#reloading = null;
		})();
		return this.#reloading;
	}

	async #reloadOnce(): Promise<void> {
		for (const tab of [...this.all]) {
			if (this.isDirty(tab)) continue;
			const id = tab.id;
			try {
				const request = await api.requests.get(id);
				// The user may have started editing, or the tab moved, while we waited.
				if (tab.id !== id || this.isDirty(tab)) continue;
				if (JSON.stringify(request) !== JSON.stringify(tab.saved)) {
					tab.saved = request;
					tab.draft = clone(request);
				}
			} catch (err) {
				// Only a request that is gone closes its tab; anything else is worth knowing about.
				const error = toApiError(err);
				if (error.kind !== 'notFound') {
					toast.error(error.message);
					continue;
				}
				const index = this.all.indexOf(tab);
				if (index === -1 || this.isDirty(tab)) continue;
				this.all.splice(index, 1);
				if (this.activeId === tab.id) this.activeId = this.all[0]?.id ?? null;
			}
		}
	}
}

function clone(request: Request): Request {
	return structuredClone($state.snapshot(request));
}

export const tabs = new TabsState();
