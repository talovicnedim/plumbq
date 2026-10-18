import { api, type Environment, type WorkspaceWarning } from '#lib/api/index.js';
import { Query } from './query.svelte.js';

/** The workspace's environments and which one requests are sent with. */
class EnvironmentsState {
	// Every refresh drops a selection whose environment is gone (deleted here, by git or by an
	// agent), so sends never go out with a dead name.
	list = new Query(async () => {
		const [list, skipped] = await Promise.all([
			api.environments.list(),
			api.environments.warnings()
		]);
		this.skipped = skipped;
		if (this.selected && !list.some((env) => env.name === this.selected)) this.select(null);
		return list;
	});
	/** Workspace-wide variable defaults from the root `folder.toml`; environments override them. */
	defaults = new Query(() =>
		api.folders.getSettings('').then((settings) => settings.variables ?? {})
	);
	/** Environment files left out of `list` because of their name. */
	skipped = $state<WorkspaceWarning[]>([]);
	/** Name of the selected environment, remembered per workspace on this machine. */
	selected = $state<string | null>(null);
	#root: string | null = null;

	readonly current: Environment | undefined = $derived(
		this.list.data?.find((env) => env.name === this.selected)
	);

	async load(root: string): Promise<void> {
		this.#root = root;
		this.selected = localStorage.getItem(this.#key()) || null;
		await Promise.all([this.list.refresh(), this.defaults.refresh()]);
	}

	/** Saves the workspace defaults, keeping the root folder's order and headers. */
	async saveDefaults(variables: Record<string, string>): Promise<void> {
		const settings = await api.folders.getSettings('');
		await api.folders.updateSettings('', { ...settings, variables });
		await this.defaults.refresh();
	}

	select(name: string | null): void {
		this.selected = name;
		if (name) localStorage.setItem(this.#key(), name);
		else localStorage.removeItem(this.#key());
	}

	#key(): string {
		return `environment:${this.#root}`;
	}
}

export const environments = new EnvironmentsState();
