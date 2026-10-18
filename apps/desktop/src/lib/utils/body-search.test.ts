import { describe, expect, it } from 'vitest';
import { MAX_MATCHES, countLabel, findMatches, searchMode, stepIndex } from './body-search.js';

describe('searchMode', () => {
	it('searches nothing for an empty or blank box', () => {
		expect(searchMode('', true)).toEqual({ kind: 'none' });
		expect(searchMode('   ', false)).toEqual({ kind: 'none' });
	});

	it('finds plain text', () => {
		expect(searchMode('endp', true)).toEqual({ kind: 'find', query: 'endp' });
		expect(searchMode('  two words ', false)).toEqual({ kind: 'find', query: 'two words' });
	});

	it('filters with a JSON path in JSON bodies only', () => {
		expect(searchMode('$.items[*].name', true)).toEqual({ kind: 'path', path: '$.items[*].name' });
		expect(searchMode('$.items', false)).toEqual({ kind: 'find', query: '$.items' });
	});
});

describe('findMatches', () => {
	it('ignores case and returns every range in order', () => {
		const { ranges, capped } = findMatches('Endpoint, endpoint, ENDPOINT', 'endp');
		expect(ranges).toEqual([
			{ from: 0, to: 4 },
			{ from: 10, to: 14 },
			{ from: 20, to: 24 }
		]);
		expect(capped).toBe(false);
	});

	it('treats the query as plain text, not a pattern', () => {
		expect(findMatches('a.b axb', 'a.b').ranges).toEqual([{ from: 0, to: 3 }]);
		expect(findMatches('cost $5 (net)', '(net)').ranges).toEqual([{ from: 8, to: 13 }]);
	});

	it('returns no ranges for a missing word', () => {
		expect(findMatches('{"id": 1}', 'name')).toEqual({ ranges: [], capped: false });
	});

	it('stops at the limit and says it was cut', () => {
		const text = 'a'.repeat(MAX_MATCHES + 50);
		const { ranges, capped } = findMatches(text, 'a');
		expect(ranges).toHaveLength(MAX_MATCHES);
		expect(capped).toBe(true);
	});

	it('is not capped when matches fit the limit exactly', () => {
		const { ranges, capped } = findMatches('a'.repeat(MAX_MATCHES), 'a');
		expect(ranges).toHaveLength(MAX_MATCHES);
		expect(capped).toBe(false);
	});
});

describe('stepIndex', () => {
	it('moves forward and wraps to the first match', () => {
		expect(stepIndex(0, 3, 1)).toBe(1);
		expect(stepIndex(2, 3, 1)).toBe(0);
	});

	it('moves back and wraps to the last match', () => {
		expect(stepIndex(1, 3, -1)).toBe(0);
		expect(stepIndex(0, 3, -1)).toBe(2);
	});

	it('stays at zero without matches', () => {
		expect(stepIndex(0, 0, 1)).toBe(0);
		expect(stepIndex(0, 0, -1)).toBe(0);
	});
});

describe('countLabel', () => {
	it('shows the position out of the total', () => {
		expect(countLabel(1, 5, false)).toBe('2 of 5');
	});

	it('marks a cut count with a plus', () => {
		expect(countLabel(0, MAX_MATCHES, true)).toBe(`1 of ${MAX_MATCHES}+`);
	});

	it('says when nothing matches', () => {
		expect(countLabel(0, 0, false)).toBe('No matches');
	});
});
