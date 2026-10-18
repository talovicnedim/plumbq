import { describe, expect, it } from 'vitest';
import type { RunOutcome } from '#lib/api/index.js';
import { addOutcome, emptyResults, parseTags, sentCount } from './runner.js';

function outcome(overrides: Partial<RunOutcome>): RunOutcome {
	return {
		id: 'a.http',
		name: 'A',
		method: 'GET',
		iteration: 1,
		status: 200,
		durationMs: 5,
		assertions: [],
		error: null,
		skipped: null,
		needsConfirmation: false,
		promptsNeeded: [],
		historyId: null,
		state: 'passed',
		...overrides
	};
}

describe('runner results', () => {
	it('counts every outcome but lists only as many as core keeps', () => {
		const results = emptyResults();
		addOutcome(results, outcome({}), 2);
		addOutcome(results, outcome({ state: 'failed' }), 2);
		addOutcome(results, outcome({ state: 'skipped' }), 2);
		addOutcome(results, outcome({ state: 'failed' }), 2);
		expect(results.outcomes.map((o) => o.state)).toEqual(['passed', 'failed']);
		expect(results).toMatchObject({ passed: 1, failed: 2, skipped: 1, dropped: 2 });
		expect(sentCount(results)).toBe(4);
	});

	it('parses tags', () => {
		expect(parseTags(' smoke, auth  users ')).toEqual(['smoke', 'auth', 'users']);
	});
});
