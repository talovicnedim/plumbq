import { describe, expect, it } from 'vitest';
import { summarize } from './assertions.js';

describe('assertion summary', () => {
	it('counts passed results', () => {
		const summary = summarize([
			{ expression: 'status == 200', passed: true, message: 'got 200' },
			{ expression: 'body.$.id == 1', passed: false, message: 'expected 1, got 2' }
		]);
		expect(summary).toEqual({ passed: 1, total: 2, label: '1/2 passed', allPassed: false });
	});

	it('is empty without assertions or for old history', () => {
		expect(summarize(undefined)).toEqual({ passed: 0, total: 0, label: '', allPassed: true });
	});
});
