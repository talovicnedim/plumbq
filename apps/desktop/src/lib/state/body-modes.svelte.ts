import { isBodyMode, modeKey } from '#lib/utils/body-modes.js';
import type { BodyMode } from '#lib/utils/preview.js';

const STORAGE_KEY = 'plumbq.bodyModes';

/** Pretty / Raw / Preview, remembered per content type on this machine. */
class BodyModesState {
	byType = $state<Record<string, BodyMode>>(load());

	get(contentType: string | undefined): BodyMode | undefined {
		return this.byType[modeKey(contentType)];
	}

	set(contentType: string | undefined, mode: BodyMode): void {
		this.byType = { ...this.byType, [modeKey(contentType)]: mode };
		localStorage.setItem(STORAGE_KEY, JSON.stringify(this.byType));
	}
}

function load(): Record<string, BodyMode> {
	try {
		const saved: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}');
		if (typeof saved !== 'object' || saved === null) return {};
		return Object.fromEntries(Object.entries(saved).filter(([, mode]) => isBodyMode(mode)));
	} catch {
		return {};
	}
}

export const bodyModes = new BodyModesState();
