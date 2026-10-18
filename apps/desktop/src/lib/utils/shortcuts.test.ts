import { describe, expect, it } from 'vitest';
import { labelFor, matchShortcut } from './shortcuts.js';

const key = (k: string, extra: Partial<KeyboardEvent> = {}) => ({
	key: k,
	metaKey: false,
	ctrlKey: false,
	shiftKey: false,
	altKey: false,
	...extra
});

describe('shortcut matching', () => {
	it('uses ⌘ on macOS and Ctrl elsewhere', () => {
		expect(matchShortcut(key('k', { metaKey: true }), true)?.id).toBe('palette');
		expect(matchShortcut(key('k', { ctrlKey: true }), true)).toBeNull();
		expect(matchShortcut(key('k', { ctrlKey: true }), false)?.id).toBe('palette');
	});

	it('leaves menu shortcuts to the native menu on macOS', () => {
		expect(matchShortcut(key('n', { metaKey: true }), true)).toBeNull();
		expect(matchShortcut(key('n', { ctrlKey: true }), false)?.id).toBe('newRequest');
	});

	it('tells shift variants apart', () => {
		expect(matchShortcut(key('W', { metaKey: true, shiftKey: true }), true)?.id).toBe(
			'closeAllTabs'
		);
		expect(matchShortcut(key('w', { ctrlKey: true }), false)?.id).toBe('closeTab');
	});

	it('maps digits to tab indexes and Esc to cancel', () => {
		expect(matchShortcut(key('3', { metaKey: true }), true)).toEqual({ id: 'selectTab', index: 2 });
		expect(matchShortcut(key('Escape'), true)).toEqual({ id: 'cancel' });
		expect(matchShortcut(key('Escape', { metaKey: true }), true)).toBeNull();
	});

	it('ignores alt combinations and plain keys', () => {
		expect(matchShortcut(key('k', { metaKey: true, altKey: true }), true)).toBeNull();
		expect(matchShortcut(key('k'), true)).toBeNull();
	});

	it('labels shortcuts per platform', () => {
		expect(labelFor('send', true)).toBe('⌘↵');
		expect(labelFor('closeAllTabs', true)).toBe('⇧⌘W');
		expect(labelFor('closeAllTabs', false)).toBe('Ctrl+Shift+W');
		expect(labelFor('selectTab', false)).toBe('Ctrl+1-9');
	});
});
