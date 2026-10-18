import { isMac } from './platform.js';

/**
 * The app's command key: ⌘ on macOS, Ctrl elsewhere. On macOS Ctrl+K, Ctrl+N and friends are
 * text-editing keys and must keep working in inputs and the editor.
 */
function hasCommandKey(e: Pick<KeyboardEvent, 'metaKey' | 'ctrlKey'>, mac = isMac): boolean {
	return mac ? e.metaKey : e.ctrlKey;
}

/** Whether a modal dialog is open; global shortcuts then stay out of the way. */
export function dialogOpen(doc: Pick<Document, 'querySelector'> = document): boolean {
	return (
		doc.querySelector(
			'[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"]'
		) !== null
	);
}

export type ShortcutId =
	| 'send'
	| 'save'
	| 'cancel'
	| 'palette'
	| 'recent'
	| 'switchWorkspace'
	| 'newRequest'
	| 'openWorkspace'
	| 'closeTab'
	| 'closeAllTabs'
	| 'selectTab'
	| 'settings'
	| 'shortcuts'
	| 'aiMcp'
	| 'assistant';

export type ShortcutGroup = 'Requests' | 'Navigation' | 'Tabs' | 'App';

export interface Shortcut {
	id: ShortcutId;
	/** `KeyboardEvent.key`, lower case for letters; `1-9` stands for any digit. */
	key: string;
	shift?: boolean;
	/** On macOS the native menu already handles it (its key equivalent swallows the event). */
	menuOnMac?: boolean;
	group: ShortcutGroup;
	label: string;
}

/** Every app shortcut, in cheat-sheet order. The handlers in the layouts dispatch on `id`. */
export const SHORTCUTS: Shortcut[] = [
	{ id: 'send', key: 'Enter', group: 'Requests', label: 'Send the request' },
	{ id: 'cancel', key: '.', group: 'Requests', label: 'Cancel a running send (also Esc)' },
	{ id: 'save', key: 's', group: 'Requests', label: 'Save the request' },
	{ id: 'newRequest', key: 'n', menuOnMac: true, group: 'Requests', label: 'New request' },
	{ id: 'palette', key: 'k', group: 'Navigation', label: 'Search requests and commands' },
	{ id: 'recent', key: 'e', group: 'Navigation', label: 'Recent requests' },
	{ id: 'switchWorkspace', key: 'p', group: 'Navigation', label: 'Switch workspace' },
	{
		id: 'openWorkspace',
		key: 'o',
		menuOnMac: true,
		group: 'Navigation',
		label: 'Open workspace folder'
	},
	{ id: 'selectTab', key: '1-9', group: 'Tabs', label: 'Go to tab 1-9' },
	{ id: 'closeTab', key: 'w', menuOnMac: true, group: 'Tabs', label: 'Close tab' },
	{ id: 'closeAllTabs', key: 'w', shift: true, group: 'Tabs', label: 'Close all tabs' },
	{ id: 'settings', key: ',', menuOnMac: true, group: 'App', label: 'Settings' },
	{ id: 'shortcuts', key: '/', group: 'App', label: 'Keyboard shortcuts' },
	{ id: 'aiMcp', key: 'i', shift: true, group: 'App', label: 'AI settings' },
	{ id: 'assistant', key: 'j', group: 'App', label: 'Show or hide the chat' }
];

interface ShortcutMatch {
	id: ShortcutId;
	/** For `selectTab`: the zero-based tab index. */
	index?: number;
}

type KeyEvent = Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey'>;

/** The shortcut a key event triggers, if any. Esc alone maps to `cancel`. */
export function matchShortcut(e: KeyEvent, mac = isMac): ShortcutMatch | null {
	if (e.key === 'Escape' && !e.metaKey && !e.ctrlKey && !e.altKey) return { id: 'cancel' };
	if (!hasCommandKey(e, mac) || e.altKey) return null;
	const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
	for (const shortcut of SHORTCUTS) {
		if (mac && shortcut.menuOnMac) continue;
		if (Boolean(shortcut.shift) !== e.shiftKey) continue;
		if (shortcut.key === '1-9' && /^[1-9]$/.test(key)) {
			return { id: shortcut.id, index: Number(key) - 1 };
		}
		if (shortcut.key === key) return { id: shortcut.id };
	}
	return null;
}

const MAC_KEYS: Record<string, string> = { Enter: '↵', ',': ',', '.': '.', '/': '/' };

/** How a shortcut is written on this platform: `⇧⌘W` on macOS, `Ctrl+Shift+W` elsewhere. */
export function shortcutLabel(shortcut: Pick<Shortcut, 'key' | 'shift'>, mac = isMac): string {
	const key =
		shortcut.key === '1-9'
			? '1-9'
			: shortcut.key.length === 1
				? shortcut.key.toUpperCase()
				: shortcut.key;
	if (mac) return `${shortcut.shift ? '⇧' : ''}⌘${MAC_KEYS[shortcut.key] ?? key}`;
	return ['Ctrl', shortcut.shift ? 'Shift' : '', key].filter(Boolean).join('+');
}

/** The label of a shortcut by id, for buttons and menus. */
export function labelFor(id: ShortcutId, mac = isMac): string {
	const shortcut = SHORTCUTS.find((s) => s.id === id);
	return shortcut ? shortcutLabel(shortcut, mac) : '';
}
