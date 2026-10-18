import { describe, expect, it } from 'vitest';
import { isBodyMode, modeKey } from './body-modes.js';

describe('body view modes', () => {
	it('remembers choices by media type, grouping image, audio and video', () => {
		expect(modeKey('application/json; charset=utf-8')).toBe('application/json');
		expect(modeKey('image/png')).toBe('image/*');
		expect(modeKey(undefined)).toBe('unknown');
	});

	it('accepts only known modes', () => {
		expect(isBodyMode('raw')).toBe(true);
		expect(isBodyMode('table')).toBe(false);
	});
});
