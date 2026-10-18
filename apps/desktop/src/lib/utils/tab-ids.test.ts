import { describe, expect, it } from 'vitest';
import { idAfterChanges, isInFolder } from './tab-ids.js';

describe('tab ids', () => {
	it('follows the changes core reports, in order', () => {
		const changes = [
			{ from: 'all.http#2', to: 'users/two.http' },
			{ from: 'all.http#3', to: 'all.http#2' }
		];
		expect(idAfterChanges('all.http#2', changes)).toBe('users/two.http');
		expect(idAfterChanges('all.http#3', changes)).toBe('all.http#2');
		expect(idAfterChanges('all.http', changes)).toBe('all.http');
	});

	it('says when a request was removed', () => {
		expect(idAfterChanges('gone.http', [{ from: 'gone.http', to: null }])).toBeNull();
	});

	it('knows which ids are inside a folder', () => {
		expect(isInFolder('users/admin/x.http', 'users')).toBe(true);
		expect(isInFolder('users2/x.http', 'users')).toBe(false);
	});
});
