export type Theme = 'system' | 'light' | 'dark';
export type Density = 'comfortable' | 'compact';

export interface Settings {
	theme: Theme;
	/** Compact trims padding in lists and editors for small screens. */
	density: Density;
	/** Code editor font size in px. */
	editorFontSize: number;
	/** Wrap long lines in body and response editors by default. */
	wordWrap: boolean;
	tabSize: number;
	/** Pretty-print JSON pasted into an empty JSON body. */
	formatJsonOnPaste: boolean;
	/** Save edited requests after a second without typing. */
	autoSave: boolean;
	checkForUpdates: boolean;
	/** The welcome tips in the empty editor area. */
	showTips: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
	theme: 'system',
	density: 'comfortable',
	editorFontSize: 13,
	wordWrap: true,
	tabSize: 2,
	formatJsonOnPaste: true,
	autoSave: false,
	checkForUpdates: true,
	showTips: true
};

export const FONT_SIZES = [11, 12, 13, 14, 15, 16];
export const TAB_SIZES = [2, 4];

/**
 * Saved settings over the defaults. Unknown keys and values of the wrong type (from older or
 * hand-edited storage) fall back to the default, so a bad entry never breaks the app.
 */
export function mergeSettings(saved: unknown): Settings {
	if (typeof saved !== 'object' || saved === null) return { ...DEFAULT_SETTINGS };
	const merged = { ...DEFAULT_SETTINGS };
	for (const [key, fallback] of Object.entries(DEFAULT_SETTINGS)) {
		const value: unknown = key in saved ? Reflect.get(saved, key) : undefined;
		if (typeof value === typeof fallback) {
			Object.assign(merged, { [key]: value });
		}
	}
	if (!['system', 'light', 'dark'].includes(merged.theme)) merged.theme = DEFAULT_SETTINGS.theme;
	if (!['comfortable', 'compact'].includes(merged.density))
		merged.density = DEFAULT_SETTINGS.density;
	if (!FONT_SIZES.includes(merged.editorFontSize))
		merged.editorFontSize = DEFAULT_SETTINGS.editorFontSize;
	if (!TAB_SIZES.includes(merged.tabSize)) merged.tabSize = DEFAULT_SETTINGS.tabSize;
	return merged;
}
