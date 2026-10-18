import { toast } from 'svelte-sonner';
import { api, toApiError, type WorkspaceSummary } from '#lib/api/index.js';
import { assistant } from './assistant.svelte.js';
import { prompt } from './prompt.svelte.js';
import { confirmDiscardAll } from './quit.js';
import { tabs } from './tabs.svelte.js';
import { Query } from './query.svelte.js';

/** The open workspace folder, its request tree, and recently opened folders. */
class WorkspaceState {
	root = $state<string | null>(null);
	/** What the app calls the open workspace: its display name, or the folder's name. */
	name = $state('');
	recent = $state<WorkspaceSummary[]>([]);
	loaded = $state(false);
	tree = new Query(() => api.workspace.tree());
	/** Folder paths collapsed in the sidebar. */
	collapsed = $state(new Set<string>());

	/** Why loading at start failed, shown on the Welcome screen with a way to try again. */
	problem = $state<string>();

	async load(): Promise<void> {
		this.problem = undefined;
		try {
			this.#show(await api.workspace.current());
			this.recent = await api.workspace.recent();
		} catch (err) {
			this.problem = toApiError(err).message;
			// The Welcome screen shows the problem; an open workspace hides it.
			if (this.root) toast.error(this.problem);
		}
		this.loaded = true;
		if (this.root) await this.tree.refresh();
	}

	open(path: string): Promise<boolean> {
		return this.#switch(() => api.workspace.open(path));
	}

	/** Opens the demo workspace (created on first use in the app's data folder). */
	openDemo(): Promise<boolean> {
		return this.#switch(() => api.workspace.openDemo());
	}

	/** Asks where and under which name, then creates and opens a new workspace folder. */
	async create(): Promise<boolean> {
		const parent = await api.dialog.open('workspaceParent');
		if (!parent) return false;
		const name = await prompt({
			title: 'New workspace',
			label: `Folder name (created in ${parent})`,
			confirmLabel: 'Create',
			placeholder: 'my-api'
		});
		if (!name) return false;
		return this.#switch(() => api.workspace.create(parent, name));
	}

	/** Asks for the name to show; the folder on disk keeps its name. */
	async rename(): Promise<void> {
		const name = await prompt({
			title: 'Rename workspace',
			label: 'Shown in Plumbq and saved in folder.toml. The folder keeps its name.',
			confirmLabel: 'Rename',
			initial: this.name
		});
		if (name === null) return;
		try {
			this.#show(await api.workspace.rename(name));
		} catch (err) {
			toast.error(toApiError(err).message);
			return;
		}
		await this.#loadRecent();
	}

	/** `folder.toml` changed on disk (git, an editor): the name may have changed with it. */
	async refreshName(): Promise<void> {
		const current = await api.workspace.current();
		if (current && current.root === this.root) this.name = current.name;
	}

	#show(opened: WorkspaceSummary | null): void {
		this.root = opened?.root ?? null;
		this.name = opened?.name ?? '';
	}

	async #switch(openRoot: () => Promise<WorkspaceSummary>): Promise<boolean> {
		// Another workspace has other requests. Ask about edits first, but close the tabs only
		// once it opened, so a failed open keeps them.
		if (!(await confirmDiscardAll('Switch workspace'))) return false;
		try {
			this.#show(await openRoot());
		} catch (err) {
			toast.error(toApiError(err).message);
			return false;
		}
		tabs.dropAll();
		assistant.reset();
		this.collapsed = new Set();
		await Promise.all([this.#loadRecent(), this.tree.refresh()]);
		return true;
	}

	async #loadRecent(): Promise<void> {
		try {
			this.recent = await api.workspace.recent();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	/** Asks for a folder with the native picker. */
	async choose(): Promise<boolean> {
		const path = await api.dialog.open('workspace');
		return path ? this.open(path) : false;
	}

	toggle(path: string): void {
		const next = new Set(this.collapsed);
		if (!next.delete(path)) next.add(path);
		this.collapsed = next;
	}
}

export const workspace = new WorkspaceState();
