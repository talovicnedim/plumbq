import { toast } from 'svelte-sonner';
import { api, toApiError, type ImportFormat, type ImportSummary } from '#lib/api/index.js';
import { toastCurlSecrets } from './curl-secrets.js';
import { environments } from './environments.svelte.js';
import { tabs } from './tabs.svelte.js';
import { workspace } from './workspace.svelte.js';

type ImportSource = 'curl' | 'file';

const FORMAT_LABEL: Record<ImportFormat, string> = {
	postman: 'Postman collection',
	openApi: 'OpenAPI spec',
	swagger: 'Swagger spec',
	insomnia: 'Insomnia export',
	har: 'HAR recording',
	bruno: 'Bruno collection'
};

/** The Import dialog, plus importing files dropped on the window. */
class ImporterState {
	open = $state(false);
	source = $state<ImportSource>('curl');
	/** Workspace folder to import into; `""` is the root. */
	folder = $state('');
	busy = $state(false);

	show(folder: string, source: ImportSource): void {
		this.folder = folder;
		this.source = source;
		this.open = true;
	}

	async curl(name: string, command: string): Promise<boolean> {
		return this.#run(async () => {
			const environment = environments.selected;
			const { id, secrets } = await api.requests.importCurl(
				this.folder,
				name,
				command,
				environment
			);
			await workspace.tree.refresh();
			await tabs.open(id);
			toast.success(`Imported "${name}" from cURL`);
			toastCurlSecrets(secrets, environment);
		});
	}

	/** Lets the user pick a file, then imports it. */
	async browse(): Promise<boolean> {
		const path = await api.dialog.open('importFile');
		return path ? this.file(path) : false;
	}

	/** Bruno collections are folders. */
	async browseFolder(): Promise<boolean> {
		const path = await api.dialog.open('importFolder');
		return path ? this.file(path) : false;
	}

	/** A file, or a Bruno collection folder. */
	async file(path: string): Promise<boolean> {
		return this.#run(async () => report(await api.import.file(this.folder, path)));
	}

	/** Downloads a spec, e.g. `https://petstore3.swagger.io/api/v3/openapi.json`. */
	async url(url: string): Promise<boolean> {
		return this.#run(async () => report(await api.import.url(this.folder, url)));
	}

	async #run(work: () => Promise<void>): Promise<boolean> {
		this.busy = true;
		try {
			await work();
			this.open = false;
			return true;
		} catch (err) {
			toast.error(toApiError(err).message);
			return false;
		} finally {
			this.busy = false;
		}
	}
}

export const importer = new ImporterState();

/** Refreshes the tree and tells the user what happened, including what a re-import kept. */
async function report(summary: ImportSummary): Promise<void> {
	await workspace.tree.refresh();
	const label = FORMAT_LABEL[summary.format];
	const message = summary.merged
		? `Updated ${summary.folder} from ${label}: ${summary.added} new, ${summary.updated} renamed, ${summary.unchanged} unchanged`
		: `Imported ${label}: ${summary.added} requests into ${summary.folder}`;
	const details = [...summary.notes];
	if (summary.stale.length > 0) {
		details.push(`${summary.stale.length} requests are no longer in the source; they were kept`);
	}
	if (summary.environments.length > 0) {
		details.push(`environment: ${summary.environments.join(', ')}`);
	}
	toast.success(message, { description: details.join('. ') || undefined });
}
