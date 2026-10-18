import { setMode } from 'mode-watcher';
import { mergeSettings, type Settings } from '#lib/utils/settings.js';

const STORAGE_KEY = 'plumbq.settings';

/** App preferences, kept on this machine (workspace settings live in `folder.toml`). */
class SettingsState {
	values = $state<Settings>(load());

	update(patch: Partial<Settings>): void {
		this.values = { ...this.values, ...patch };
		localStorage.setItem(STORAGE_KEY, JSON.stringify(this.values));
		if (patch.theme) setMode(patch.theme);
	}
}

function load(): Settings {
	try {
		return mergeSettings(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}'));
	} catch {
		return mergeSettings(null);
	}
}

export const settings = new SettingsState();
