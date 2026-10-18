import type { TreeNode } from '#lib/api/index.js';

// Sizes are in rem, like the chevron (`size-4`) and the row padding (`px-2`), so the guides
// stay under the chevrons when the density setting changes the root font size.
const ROW_EDGE = 0.5;
const CHEVRON = 1;

/** Requests inside a folder at any depth. */
export function countRequests(node: TreeNode): number {
	if (node.kind === 'request') return 1;
	return node.children.reduce((sum, child) => sum + countRequests(child), 0);
}

/** The method as shown in the narrow method column. */
export function shortMethod(method: string): string {
	if (method === 'DELETE') return 'DEL';
	if (method === 'OPTIONS') return 'OPT';
	return method;
}

/** Left padding of a row, in rem: a child starts one chevron width after its folder. */
export function rowIndent(depth: number): number {
	return ROW_EDGE + depth * CHEVRON;
}

/** Left offsets of a row's indent guides, in rem: one under the chevron of each ancestor. */
export function guideOffsets(depth: number): number[] {
	return Array.from({ length: depth }, (_, level) => rowIndent(level) + CHEVRON / 2);
}
