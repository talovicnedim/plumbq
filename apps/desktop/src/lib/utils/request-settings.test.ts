import { describe, expect, it } from 'vitest';
import { hasRequestSettings } from './request-settings.js';

describe('hasRequestSettings', () => {
	it('is false for missing, empty or all-off settings', () => {
		expect(hasRequestSettings(undefined)).toBe(false);
		expect(hasRequestSettings({})).toBe(false);
		expect(hasRequestSettings({ noRedirect: false, timeoutSecs: null })).toBe(false);
	});

	it('is true once anything is turned on or set', () => {
		expect(hasRequestSettings({ insecure: true })).toBe(true);
		expect(hasRequestSettings({ timeoutSecs: 0 })).toBe(true);
	});
});
