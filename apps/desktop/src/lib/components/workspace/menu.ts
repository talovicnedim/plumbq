import type { TreeNode } from '#lib/api/index.js';
import {
	copyCurl,
	duplicateRequest,
	importCurl,
	newFolder,
	newRequest,
	removeFolder,
	removeRequest,
	renameFolder,
	duplicateFolder,
	renameRequest,
	reveal
} from '#lib/state/actions.js';
import { importer } from '#lib/state/importer.svelte.js';
import { exportPostman, runner } from '#lib/state/runner.svelte.js';
import { revealLabel } from '#lib/utils/platform.js';

interface MenuEntry {
	label: string;
	run: () => unknown;
	destructive?: boolean;
	/** Draw a separator above this entry. */
	separated?: boolean;
}

/** Actions for a tree node, shared by its right-click menu and its "…" button. */
export function nodeMenu(node: TreeNode): MenuEntry[] {
	if (node.kind === 'folder') {
		return [
			{ label: 'New request', run: () => newRequest(node.path) },
			{ label: 'New folder', run: () => newFolder(node.path) },
			{ label: 'Import cURL…', run: () => importCurl(node.path) },
			{ label: 'Import file…', run: () => importer.show(node.path, 'file') },
			{ label: 'Run folder…', run: () => runner.show(node.path), separated: true },
			{ label: 'Export to Postman…', run: () => exportPostman(node.path) },
			{ label: 'Rename…', run: () => renameFolder(node.path, node.name), separated: true },
			{ label: 'Duplicate', run: () => duplicateFolder(node.path) },
			{ label: revealLabel, run: () => reveal(node.path) },
			{
				label: 'Delete',
				run: () => removeFolder(node.path, node.name),
				destructive: true,
				separated: true
			}
		];
	}
	return [
		{ label: 'Rename…', run: () => renameRequest(node.id, node.name) },
		{ label: 'Duplicate', run: () => duplicateRequest(node.id) },
		{ label: 'Copy as cURL', run: () => copyCurl(node.id) },
		{ label: revealLabel, run: () => reveal(node.id) },
		{
			label: 'Delete',
			run: () => removeRequest(node.id, node.name),
			destructive: true,
			separated: true
		}
	];
}
