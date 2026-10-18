import { describe, expect, it } from 'vitest';
import { formatExpiry } from './cookies.js';

describe('formatExpiry', () => {
	const now = new Date('2026-10-11T12:00:00Z');

	it('labels session and expired cookies', () => {
		expect(formatExpiry(null, now)).toBe('session');
		expect(formatExpiry('2026-10-11T11:59:59Z', now)).toBe('expired');
	});

	it('shows a date for later days', () => {
		const later = '2026-12-24T12:00:00Z';
		expect(formatExpiry(later, now)).toBe(new Date(later).toLocaleDateString());
	});
});
