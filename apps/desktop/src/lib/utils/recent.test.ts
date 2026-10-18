import { describe, expect, it } from 'vitest';
import { MAX_RECENT, renameRecent, touchRecent } from './recent.js';

describe('recent requests', () => {
	it('moves touched requests to the front and keeps send details', () => {
		let list = touchRecent([], 'a.http', {}, 1);
		list = touchRecent(list, 'b.http', { status: 200, sentAt: 2 }, 2);
		list = touchRecent(list, 'b.http', {}, 3);
		list = touchRecent(list, 'a.http', {}, 4);
		expect(list.map((r) => r.id)).toEqual(['a.http', 'b.http']);
		expect(list[1]).toMatchObject({ status: 200, sentAt: 2, at: 3 });
	});

	it('keeps a bounded list', () => {
		let list = touchRecent([], 'x0');
		for (let i = 1; i < MAX_RECENT + 5; i++) list = touchRecent(list, `x${i}`);
		expect(list).toHaveLength(MAX_RECENT);
		expect(list[0].id).toBe(`x${MAX_RECENT + 4}`);
	});

	it('follows moves and forgets removed requests', () => {
		const list = [
			{ id: 'a.http', at: 1 },
			{ id: 'b.http', at: 2 }
		];
		expect(
			renameRecent(list, [
				{ from: 'a.http', to: 'users/a.http' },
				{ from: 'b.http', to: null }
			]).map((r) => r.id)
		).toEqual(['users/a.http']);
	});
});
