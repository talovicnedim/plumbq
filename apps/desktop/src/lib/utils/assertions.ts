import type { AssertionResult } from '#lib/api/index.js';

interface AssertionSummary {
	passed: number;
	total: number;
	/** `"3/4 passed"`, or `""` when there were no assertions. */
	label: string;
	allPassed: boolean;
}

/** Counts for the response status bar. `undefined` covers history saved before assertions. */
export function summarize(results: AssertionResult[] | undefined): AssertionSummary {
	const list = results ?? [];
	const passed = list.filter((r) => r.passed).length;
	return {
		passed,
		total: list.length,
		label: list.length ? `${passed}/${list.length} passed` : '',
		allPassed: passed === list.length
	};
}
