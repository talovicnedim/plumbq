import { describe, expect, it } from 'vitest';
import { fromBulk, toBulk } from './bulk.js';

describe('bulk edit', () => {
	it('round-trips headers keeping order and colons in values', () => {
		const rows = [
			{ name: 'Accept', value: 'application/json' },
			{ name: 'X-Time', value: '12:30' }
		];
		expect(toBulk(rows, ':')).toBe('Accept: application/json\nX-Time: 12:30');
		expect(fromBulk(toBulk(rows, ':'), ':')).toEqual(rows);
	});

	it('parses params, skipping blank lines', () => {
		expect(fromBulk('page=2\n\nflag\nq={{term}}=x', '=')).toEqual([
			{ name: 'page', value: '2' },
			{ name: 'flag', value: '' },
			{ name: 'q', value: '{{term}}=x' }
		]);
	});
});
