import { describe, expect, it } from 'vitest';
import type { TreeNode } from '#lib/api/index.js';
import { filterTree, flatten, matches } from './search.js';

const tree: TreeNode[] = [
	{
		kind: 'folder',
		name: 'users',
		path: 'users',
		children: [
			{
				kind: 'request',
				id: 'users/list.http',
				name: 'List users',
				method: 'GET',
				url: '/users',
				tags: ['smoke']
			}
		]
	},
	{ kind: 'request', id: 'login.http', name: 'Login', method: 'POST', url: '/login', tags: [] }
];

describe('search', () => {
	it('flattens with folder labels', () => {
		expect(flatten(tree).map((e) => `${e.folder}|${e.name}`)).toEqual([
			'users|List users',
			'|Login'
		]);
	});

	it('matches every term anywhere', () => {
		const [list] = flatten(tree);
		expect(matches(list, 'get users')).toBe(true);
		expect(matches(list, 'post users')).toBe(false);
	});

	it('filters the tree keeping parent folders', () => {
		const filtered = filterTree(tree, 'list');
		expect(filtered).toHaveLength(1);
		expect(filtered[0].kind === 'folder' && filtered[0].children).toHaveLength(1);
		expect(filterTree(tree, '')).toBe(tree);
	});

	it('filters by exact tag', () => {
		const [list, login] = flatten(tree);
		expect(matches(list, 'tag:smoke')).toBe(true);
		expect(matches(list, 'tag:smo')).toBe(false);
		expect(matches(login, 'tag:smoke')).toBe(false);
		expect(matches(list, 'smoke')).toBe(true);
	});
});
