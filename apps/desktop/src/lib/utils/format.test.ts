import { describe, expect, it } from 'vitest';
import { baseName, formatBodyChange, formatBytes, formatDuration } from './format.js';

describe('format', () => {
	it('formats sizes', () => {
		expect(formatBytes(512)).toBe('512 B');
		expect(formatBytes(2048)).toBe('2.0 KB');
		expect(formatBytes(3 * 1024 * 1024)).toBe('3.0 MB');
	});

	it('formats durations', () => {
		expect(formatDuration(87)).toBe('87 ms');
		expect(formatDuration(1530)).toBe('1.53 s');
	});

	it('takes the last path segment', () => {
		expect(baseName('/Users/ada/api/')).toBe('api');
		expect(baseName('C:\\work\\api')).toBe('api');
	});

	it('writes a changed JSON field as one readable line', () => {
		const change = (kind: 'added' | 'removed' | 'changed', before = '', after = '') => ({
			kind,
			path: '$.price',
			before,
			after
		});
		expect(formatBodyChange(change('changed', '89.9', '94.5'))).toBe('~ $.price: 89.9 → 94.5');
		expect(formatBodyChange(change('added', '', '"new"'))).toBe('+ $.price = "new"');
		expect(formatBodyChange(change('removed', '"L-1042"'))).toBe('- $.price (was "L-1042")');
	});
});
