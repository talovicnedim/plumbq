import type { IdChange } from '#lib/api/index.js';

/**
 * Request ids are positional for multi-request files (`all.http`, `all.http#2`, …) and contain
 * folder paths, so moves, removals and folder renames change them. Core reports exactly what
 * changed (`IdChange`); these helpers apply that to open tabs.
 */

/** What `id` is after `changes` (applied in order); `null` when its request was removed. */
export function idAfterChanges(id: string, changes: IdChange[]): string | null {
	let current = id;
	for (const change of changes) {
		if (change.from !== current) continue;
		if (change.to === null) return null;
		current = change.to;
	}
	return current;
}

/** Whether a request id is inside folder `path` (at any depth). */
export function isInFolder(id: string, path: string): boolean {
	return path === '' || id.startsWith(`${path}/`);
}
