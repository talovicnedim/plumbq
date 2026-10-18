import { describe, expect, it } from 'vitest';
import { treeKeyAction, type TreeRow } from './tree-keys.js';

// users/ (open) > list, get ; auth/ (closed) ; health
const rows: TreeRow[] = [
	{ kind: 'folder', depth: 0, open: true },
	{ kind: 'request', depth: 1 },
	{ kind: 'request', depth: 1 },
	{ kind: 'folder', depth: 0, open: false },
	{ kind: 'request', depth: 0 }
];

describe('tree keyboard navigation', () => {
	it('moves up and down and jumps to the ends', () => {
		expect(treeKeyAction('ArrowDown', rows, 0)).toEqual({ type: 'focus', index: 1 });
		expect(treeKeyAction('ArrowUp', rows, 0)).toBeNull();
		expect(treeKeyAction('ArrowDown', rows, 4)).toBeNull();
		expect(treeKeyAction('End', rows, 1)).toEqual({ type: 'focus', index: 4 });
	});

	it('opens closed folders and steps into open ones', () => {
		expect(treeKeyAction('ArrowRight', rows, 3)).toEqual({ type: 'toggle' });
		expect(treeKeyAction('ArrowRight', rows, 0)).toEqual({ type: 'focus', index: 1 });
		expect(treeKeyAction('ArrowRight', rows, 1)).toBeNull();
	});

	it('closes open folders and goes to the parent from children', () => {
		expect(treeKeyAction('ArrowLeft', rows, 0)).toEqual({ type: 'toggle' });
		expect(treeKeyAction('ArrowLeft', rows, 2)).toEqual({ type: 'focus', index: 0 });
		expect(treeKeyAction('ArrowLeft', rows, 4)).toBeNull();
	});

	it('renames and removes', () => {
		expect(treeKeyAction('F2', rows, 1)).toEqual({ type: 'rename' });
		expect(treeKeyAction('Delete', rows, 1)).toEqual({ type: 'remove' });
		expect(treeKeyAction('x', rows, 1)).toBeNull();
	});
});
