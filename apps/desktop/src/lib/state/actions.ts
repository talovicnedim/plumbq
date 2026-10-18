import { toast } from 'svelte-sonner';
import { api, toApiError } from '#lib/api/index.js';
import { confirm } from './confirm.svelte.js';
import { prompt } from './prompt.svelte.js';
import { environments } from './environments.svelte.js';
import { importer } from './importer.svelte.js';
import { tabs, type RequestTab } from './tabs.svelte.js';
import { workspace } from './workspace.svelte.js';

/** Runs a workspace change and reports errors. The file watcher refreshes the tree. */
async function run(change: () => Promise<unknown>): Promise<void> {
	try {
		await change();
		await workspace.tree.refresh();
	} catch (err) {
		toast.error(toApiError(err).message);
	}
}

export async function newRequest(folder: string): Promise<void> {
	const name = await prompt({
		title: 'New request',
		label: 'Name',
		confirmLabel: 'Create',
		placeholder: 'List users'
	});
	if (!name) return;
	await run(async () => {
		const id = await api.requests.create(folder, {
			name,
			method: 'GET',
			url: '',
			headers: [],
			body: '',
			assertions: [],
			comments: [],
			trailer: ''
		});
		await tabs.open(id);
	});
}

export async function newFolder(parent: string): Promise<void> {
	const name = await prompt({ title: 'New folder', label: 'Name', confirmLabel: 'Create' });
	if (name) await run(() => api.folders.create(parent, name));
}

export function importCurl(folder: string): void {
	importer.show(folder, 'curl');
}

/** Copies a folder with all its requests next to itself. */
export async function duplicateFolder(path: string): Promise<void> {
	await run(() => api.folders.duplicate(path));
}

export async function renameFolder(path: string, current: string): Promise<void> {
	const name = await prompt({
		title: 'Rename folder',
		label: 'Name',
		confirmLabel: 'Rename',
		initial: current
	});
	if (!name || name === current) return;
	// Open tabs below the folder keep their requests (and unsaved edits) under the new path.
	await run(async () => tabs.applyChanges((await api.folders.rename(path, name)).changes));
}

export async function removeFolder(path: string, name: string): Promise<void> {
	const ok = await confirm({
		title: `Delete folder "${name}"?`,
		description: 'The folder and every request in it are deleted from disk.',
		confirmLabel: 'Delete',
		destructive: true
	});
	if (!ok) return;
	// Its open tabs go too; unsaved edits there are asked about first.
	if (!(await tabs.closeFolder(path))) return;
	await run(async () => tabs.applyChanges(await api.folders.remove(path)));
}

export async function removeRequest(id: string, name: string): Promise<void> {
	const tab = tabs.all.find((t) => t.id === id);
	const ok = await confirm({
		title: `Delete "${name}"?`,
		description:
			tab && tabs.isDirty(tab)
				? 'The request is deleted from disk, and the unsaved edits in its tab are lost.'
				: 'The request is deleted from disk.',
		confirmLabel: 'Delete',
		destructive: true
	});
	if (!ok) return;
	await run(async () => {
		// What the tab read, so a file changed in the meantime doesn't lose the wrong request.
		const changes = await api.requests.remove(id, tab?.saved.source ?? null);
		tabs.drop(id);
		// Later requests of the same file move up one place; their tabs follow.
		tabs.applyChanges(changes);
	});
}

/** Drag and drop: moves a request into `folder`, before the entry at `position`. */
export async function moveRequest(
	id: string,
	folder: string,
	position: number | null
): Promise<void> {
	await run(async () => {
		const tab = tabs.all.find((t) => t.id === id);
		const moved = await api.requests.move(id, folder, position, tab?.saved.source ?? null);
		// The moved request and, after a split, the later ones of its old file.
		tabs.applyChanges(moved.changes);
	});
}

export async function renameRequest(id: string, current: string): Promise<void> {
	const name = await prompt({
		title: 'Rename request',
		label: 'Name',
		confirmLabel: 'Rename',
		initial: current
	});
	if (!name || name === current) return;
	await run(async () => {
		const tab = tabs.all.find((t) => t.id === id);
		if (tab && tabs.isDirty(tab)) await renameEdited(tab, name);
		else await renameClean(id, name);
	});
}

/**
 * Renames what the tab read, so its next save still conflicts when the file changed on disk
 * meanwhile (the rename itself fails then).
 */
async function renameEdited(tab: RequestTab, name: string): Promise<void> {
	const id = tab.id;
	const saved = await api.requests.update(id, { ...tab.saved, name }, tab.saved.source ?? null);
	tab.saved = { ...tab.saved, name, source: saved.source };
	tab.draft.name = name;
	tab.draft.source = saved.source;
}

async function renameClean(id: string, name: string): Promise<void> {
	const request = await api.requests.get(id);
	const saved = await api.requests.update(id, { ...request, name }, request.source ?? null);
	const tab = tabs.all.find((t) => t.id === id);
	if (tab && !tabs.isDirty(tab)) {
		tab.saved = saved;
		tab.draft = structuredClone(saved);
	}
}

/** Copies a request into its own file next to the original and opens it. */
export async function duplicateRequest(id: string): Promise<void> {
	const folder = id.includes('/') ? id.slice(0, id.lastIndexOf('/')) : '';
	await run(async () => {
		const request = await api.requests.get(id);
		const copy = await api.requests.create(folder, { ...request, name: `${request.name} copy` });
		await tabs.open(copy);
	});
}

export async function copyCurl(id: string): Promise<void> {
	try {
		const request = await api.requests.get(id);
		const command = await api.requests.export(id, request, environments.selected, 'curl');
		await navigator.clipboard.writeText(command);
		toast.success('Copied as cURL (secrets left as {{name}})');
	} catch (err) {
		toast.error(toApiError(err).message);
	}
}

/** Shows a request file or folder in Finder / Explorer. */
export async function reveal(relPath: string): Promise<void> {
	const root = workspace.root;
	if (!root) return;
	const file = relPath.split('#')[0];
	try {
		await api.app.reveal(file ? `${root}/${file}` : root);
	} catch (err) {
		toast.error(toApiError(err).message);
	}
}
