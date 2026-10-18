import { api, type Environment, type Header } from '#lib/api/index.js';
import { fileKey, syncDraft } from '#lib/utils/environment-draft.js';
import { confirm } from './confirm.svelte.js';
import { environments } from './environments.svelte.js';

/**
 * What the Environments dialog edits: a copy of one environment and of the workspace defaults.
 * Save writes a copy back; switching to another environment or closing asks before edits go.
 */
export class EnvironmentEditor {
	draft = $state<Environment | null>(null);
	changedOnDisk = $state(false);
	defaults = $state<Header[]>([]);
	defaultsChangedOnDisk = $state(false);
	showDefaults = $state(false);
	/** fileKey of the copy the draft started from. */
	#base = $state('');
	/** rowsKey of the saved defaults the rows started from. */
	#defaultsBase = $state('');

	readonly dirty = $derived(this.draft !== null && fileKey(this.draft) !== this.#base);
	readonly defaultsDirty = $derived(rowsKey(this.defaults) !== this.#defaultsBase);

	/** Fresh copies when the dialog opens: the environment edited last time, or the first one. */
	start(list: Environment[], defaults: Record<string, string> | undefined): void {
		this.#take(list.find((e) => e.name === this.draft?.name) ?? list[0]);
		this.#takeDefaults(toRows(defaults ?? {}));
	}

	async select(name: string, list: Environment[]): Promise<void> {
		this.showDefaults = false;
		if (name === this.draft?.name) return;
		if (this.dirty && !(await confirmDiscard())) return;
		this.#take(list.find((e) => e.name === name));
	}

	openDefaults(): void {
		this.showDefaults = true;
	}

	/** Creates an environment and opens it once the refreshed list has it. */
	async create(name: string): Promise<void> {
		const created = await api.environments.create(name, 'local');
		await environments.list.refresh();
		await this.select(created, environments.list.data ?? []);
	}

	/** Whether the dialog may close: nothing unsaved, or the user lets it go. */
	async mayClose(): Promise<boolean> {
		if (!this.dirty && !this.defaultsDirty) return true;
		return confirmDiscard();
	}

	setVariables(rows: Header[]): void {
		if (this.draft) this.draft.variables = Object.fromEntries(rows.map((r) => [r.name, r.value]));
	}

	/**
	 * Follows a refresh (the file watcher, adding a secret): keychain state changes in place, an
	 * unedited draft follows the file, an edited one is only flagged.
	 */
	follow(list: Environment[]): void {
		const draft = this.draft;
		const fresh = draft && list.find((e) => e.name === draft.name);
		if (!draft || !fresh) return this.#take(list[0]);
		draft.secrets = draft.secrets.map((s) => ({
			...s,
			isSet: fresh.secrets.find((f) => f.name === s.name)?.isSet ?? s.isSet
		}));
		draft.auth = draft.auth.map((a) => ({
			...a,
			signedIn: fresh.auth.find((f) => f.name === a.name)?.signedIn ?? false
		}));
		const sync = syncDraft(draft, fresh, this.#base);
		if (sync === 'reload') return this.#take(fresh);
		if (sync === 'conflict') {
			this.changedOnDisk = true;
			return;
		}
		this.#base = fileKey(fresh);
		this.changedOnDisk = false;
	}

	/** Like `follow`, for the workspace defaults. */
	followDefaults(saved: Record<string, string> | undefined): void {
		const fresh = toRows(saved ?? {});
		const freshKey = rowsKey(fresh);
		if (freshKey === rowsKey(this.defaults)) {
			this.#defaultsBase = freshKey;
			this.defaultsChangedOnDisk = false;
		} else if (!this.defaultsDirty) {
			this.#takeDefaults(fresh);
		} else if (freshKey !== this.#defaultsBase) {
			this.defaultsChangedOnDisk = true;
		}
	}

	#take(env: Environment | undefined): void {
		this.draft = env ? structuredClone($state.snapshot(env)) : null;
		this.#base = env ? fileKey(env) : '';
		this.changedOnDisk = false;
	}

	#takeDefaults(rows: Header[]): void {
		this.defaults = rows;
		this.#defaultsBase = rowsKey(rows);
		this.defaultsChangedOnDisk = false;
	}
}

function confirmDiscard(): Promise<boolean> {
	return confirm({
		title: 'Discard changes?',
		description: 'Your unsaved environment changes are lost.',
		confirmLabel: 'Discard',
		destructive: true
	});
}

function toRows(variables: Record<string, string>): Header[] {
	return Object.entries(variables).map(([name, value]) => ({ name, value }));
}

function rowsKey(rows: Header[]): string {
	return JSON.stringify(rows.filter((r) => r.name));
}
