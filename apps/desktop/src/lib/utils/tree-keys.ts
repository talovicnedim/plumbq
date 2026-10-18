/** A visible sidebar row, as seen by keyboard navigation. */
export interface TreeRow {
	kind: 'folder' | 'request';
	depth: number;
	/** Folders only: whether its children are shown. */
	open?: boolean;
}

type TreeKeyAction =
	{ type: 'focus'; index: number } | { type: 'toggle' } | { type: 'rename' } | { type: 'remove' };

/**
 * What a key does on the focused row of the sidebar tree (WAI-ARIA tree pattern): ↑/↓ move,
 * → opens a folder or steps into it, ← closes it or goes to the parent, Home/End jump, F2
 * renames, Delete removes. Enter isn't handled here: rows are buttons and open on Enter.
 */
export function treeKeyAction(key: string, rows: TreeRow[], index: number): TreeKeyAction | null {
	const row = rows[index];
	if (!row) return null;
	switch (key) {
		case 'ArrowDown':
			return index + 1 < rows.length ? { type: 'focus', index: index + 1 } : null;
		case 'ArrowUp':
			return index > 0 ? { type: 'focus', index: index - 1 } : null;
		case 'Home':
			return { type: 'focus', index: 0 };
		case 'End':
			return { type: 'focus', index: rows.length - 1 };
		case 'ArrowRight':
			if (row.kind !== 'folder') return null;
			if (!row.open) return { type: 'toggle' };
			return rows[index + 1]?.depth === row.depth + 1 ? { type: 'focus', index: index + 1 } : null;
		case 'ArrowLeft': {
			if (row.kind === 'folder' && row.open) return { type: 'toggle' };
			for (let i = index - 1; i >= 0; i--) {
				if (rows[i].depth < row.depth) return { type: 'focus', index: i };
			}
			return null;
		}
		case 'F2':
			return { type: 'rename' };
		case 'Delete':
			return { type: 'remove' };
		default:
			return null;
	}
}
