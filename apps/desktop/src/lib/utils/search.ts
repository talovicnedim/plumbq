import type { TreeNode } from '#lib/api/index.js';

export interface RequestEntry {
	id: string;
	name: string;
	method: string;
	url: string;
	/** Folder names from the root, e.g. `users / admin`. */
	folder: string;
	tags: string[];
}

/** Every request in the tree, in display order. */
export function flatten(nodes: TreeNode[], folder = ''): RequestEntry[] {
	return nodes.flatMap((node) => {
		if (node.kind === 'request') return [{ ...node, folder }];
		return flatten(node.children, folder ? `${folder} / ${node.name}` : node.name);
	});
}

/**
 * Case-insensitive match of every space-separated term against name, method, url, folder and
 * tags. `tag:smoke` matches only requests tagged exactly `smoke`.
 */
export function matches(entry: RequestEntry, query: string): boolean {
	const tags = entry.tags.map((t) => t.toLowerCase());
	const haystack =
		`${entry.name} ${entry.method} ${entry.url} ${entry.folder} ${tags.join(' ')}`.toLowerCase();
	return query
		.toLowerCase()
		.split(/\s+/)
		.filter(Boolean)
		.every((term) =>
			term.startsWith('tag:') ? tags.includes(term.slice(4)) : haystack.includes(term)
		);
}

/** The tree with only matching requests and the folders that contain them. */
export function filterTree(nodes: TreeNode[], query: string, folder = ''): TreeNode[] {
	if (!query.trim()) return nodes;
	return nodes.flatMap((node): TreeNode[] => {
		if (node.kind === 'request') return matches({ ...node, folder }, query) ? [node] : [];
		const children = filterTree(
			node.children,
			query,
			folder ? `${folder} / ${node.name}` : node.name
		);
		return children.length ? [{ ...node, children }] : [];
	});
}
