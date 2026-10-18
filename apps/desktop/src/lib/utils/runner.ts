import type { RunOutcome, RunReport } from '#lib/api/index.js';

/** What the Runner lists and counts: a report, or a run in progress shaped like one. */
export type RunResults = Pick<RunReport, 'outcomes' | 'passed' | 'failed' | 'skipped' | 'dropped'>;

export function emptyResults(): RunResults {
	return { outcomes: [], passed: 0, failed: 0, skipped: 0, dropped: 0 };
}

/** Counts a live outcome, and lists it while fewer than `keep` (from core) are listed. */
export function addOutcome(results: RunResults, outcome: RunOutcome, keep: number): void {
	results[outcome.state] += 1;
	if (results.outcomes.length < keep) results.outcomes.push(outcome);
	else results.dropped += 1;
}

export function sentCount(results: RunResults): number {
	return results.passed + results.failed + results.skipped;
}

/** `smoke, auth` → `['smoke', 'auth']`. */
export function parseTags(text: string): string[] {
	return text
		.split(/[\s,]+/)
		.map((tag) => tag.trim())
		.filter(Boolean);
}
