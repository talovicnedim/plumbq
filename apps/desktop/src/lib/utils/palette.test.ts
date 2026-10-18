import { describe, expect, it } from 'vitest';
import { byTag, parseQuery, rank, recentFirst, score } from './palette.js';

describe('palette search', () => {
	it('reads prefixes as modes', () => {
		expect(parseQuery('> new')).toEqual({ mode: 'commands', query: 'new' });
		expect(parseQuery('#smoke')).toEqual({ mode: 'tags', query: 'smoke' });
		expect(parseQuery('@ stag')).toEqual({ mode: 'environments', query: 'stag' });
		expect(parseQuery(' login ')).toEqual({ mode: 'all', query: 'login' });
	});

	it('prefers title starts, then words, then anywhere, then subtitles, then letters in order', () => {
		const item = { title: 'New request', subtitle: 'In the root folder', keywords: 'http file' };
		expect(score(item, 'new')).toBe(100);
		expect(score(item, 'req')).toBe(80);
		expect(score(item, 'quest')).toBe(60);
		expect(score(item, 'file')).toBe(40);
		expect(score(item, 'nreq')).toBe(20);
		expect(score(item, 'xyz')).toBe(0);
		expect(score(item, 'fle')).toBe(0);
		expect(score(item, '')).toBe(1);
	});

	it('needs every term to match, in any field', () => {
		const list = { title: 'List users', subtitle: 'users', keywords: 'GET {{baseUrl}}/users' };
		expect(score(list, 'get users')).toBeGreaterThan(0);
		expect(score(list, 'post users')).toBe(0);
	});

	it('keeps tag: as an exact tag filter', () => {
		const tagged = { title: 'List users', tags: ['smoke'] };
		expect(score(tagged, 'tag:smoke')).toBeGreaterThan(0);
		expect(score(tagged, 'tag:smoke list')).toBeGreaterThan(0);
		expect(score(tagged, 'tag:smo')).toBe(0);
		expect(score({ title: 'Login' }, 'tag:smoke')).toBe(0);
	});

	it('ranks best first and keeps priority order on ties', () => {
		const items = [
			{ title: 'Send request' },
			{ title: 'Recent requests' },
			{ title: 'Settings' },
			{ title: 'Send all' }
		];
		expect(rank(items, 'se').map((i) => i.title)).toEqual(['Send request', 'Settings', 'Send all']);
		expect(rank(items, '', 2)).toHaveLength(2);
	});

	it('finds tags by prefix, exact tags first', () => {
		const items = [
			{ title: 'A', tags: ['smoke-slow'] },
			{ title: 'B', tags: [] },
			{ title: 'C', tags: ['smoke'] }
		];
		expect(byTag(items, 'smoke').map((i) => i.title)).toEqual(['C', 'A']);
		expect(byTag(items, '').map((i) => i.title)).toEqual(['A', 'C']);
	});

	it('moves recent ids to the front in their order', () => {
		const items = [{ id: 'a' }, { id: 'b' }, { id: 'c' }, { id: 'd' }];
		expect(recentFirst(items, ['c', 'x', 'a']).map((i) => i.id)).toEqual(['c', 'a', 'b', 'd']);
	});
});
