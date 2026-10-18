import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS, mergeSettings } from './settings.js';

describe('settings', () => {
	it('falls back to defaults for missing or broken storage', () => {
		expect(mergeSettings(null)).toEqual(DEFAULT_SETTINGS);
		expect(mergeSettings('nope')).toEqual(DEFAULT_SETTINGS);
	});

	it('keeps valid saved values and drops wrong types and unknown keys', () => {
		const merged = mergeSettings({ theme: 'dark', autoSave: true, tabSize: '4', extra: 1 });
		expect(merged.theme).toBe('dark');
		expect(merged.autoSave).toBe(true);
		expect(merged.tabSize).toBe(2);
		expect('extra' in merged).toBe(false);
	});

	it('rejects values outside the allowed choices', () => {
		const merged = mergeSettings({ theme: 'neon', editorFontSize: 99, density: 'tiny' });
		expect(merged.theme).toBe('system');
		expect(merged.editorFontSize).toBe(13);
		expect(merged.density).toBe('comfortable');
	});
});
