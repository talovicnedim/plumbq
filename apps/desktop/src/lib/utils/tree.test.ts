import { describe, expect, it } from 'vitest';
import type { TreeNode } from '#lib/api/index.js';
import { countRequests, guideOffsets, rowIndent, shortMethod } from './tree.js';

function request(id: string): TreeNode {
	return { kind: 'request', id, name: id, method: 'GET', url: '', tags: [] };
}

function folder(path: string, children: TreeNode[]): TreeNode {
	return { kind: 'folder', name: path, path, children };
}

describe('countRequests', () => {
	it('counts the requests of a folder at every depth', () => {
		const users = folder('users', [
			request('users/list.http'),
			folder('users/admin', [request('users/admin/roles.http'), request('users/admin/x.http')])
		]);
		expect(countRequests(users)).toBe(3);
	});

	it('counts nothing for a folder without requests', () => {
		expect(countRequests(folder('empty', [folder('empty/inner', [])]))).toBe(0);
	});
});

describe('shortMethod', () => {
	it('shortens only the two methods that do not fit the method column', () => {
		expect(['GET', 'PATCH', 'DELETE', 'OPTIONS', 'PROPFIND'].map(shortMethod)).toEqual([
			'GET',
			'PATCH',
			'DEL',
			'OPT',
			'PROPFIND'
		]);
	});
});

describe('indent guides', () => {
	it('draw one line per ancestor and none at the top level', () => {
		expect(guideOffsets(0)).toEqual([]);
		expect(guideOffsets(3)).toHaveLength(3);
	});

	it('sit under the middle of the chevron of each ancestor', () => {
		const chevronWidth = 1;
		const offsets = guideOffsets(3);
		expect(offsets.map((left, level) => left - rowIndent(level))).toEqual(
			offsets.map(() => chevronWidth / 2)
		);
	});

	it('move a child one chevron width to the right of its folder', () => {
		expect(rowIndent(2) - rowIndent(1)).toBe(1);
	});
});
