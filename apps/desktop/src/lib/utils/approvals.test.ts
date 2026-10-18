import { describe, expect, it } from 'vitest';
import { canAllow } from './approvals.js';

describe('canAllow', () => {
	it('allows requests that need no typed name', () => {
		expect(canAllow({ strong: false, confirmName: null }, '')).toBe(true);
	});

	it('needs the exact name on strong requests, trimmed like core', () => {
		const strong = { strong: true, confirmName: 'production' };
		expect(canAllow(strong, '')).toBe(false);
		expect(canAllow(strong, 'prod')).toBe(false);
		expect(canAllow(strong, 'Production')).toBe(false);
		expect(canAllow(strong, 'production')).toBe(true);
		expect(canAllow(strong, ' production ')).toBe(true);
	});
});
