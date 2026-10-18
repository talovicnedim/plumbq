const KEY = 'layout';

interface Saved {
	sidebarWidth: number;
	assistantWidth: number;
	/** Share of the editor in the editor/response split, 0-1. */
	split: number;
	/** Side by side (columns) or stacked (rows). */
	orientation: 'columns' | 'rows';
}

const DEFAULTS: Saved = { sidebarWidth: 288, assistantWidth: 380, split: 0.5, orientation: 'rows' };

/** Pane sizes, remembered on this machine. */
class LayoutState {
	sidebarWidth = $state(DEFAULTS.sidebarWidth);
	assistantWidth = $state(DEFAULTS.assistantWidth);
	split = $state(DEFAULTS.split);
	orientation = $state<Saved['orientation']>(DEFAULTS.orientation);

	constructor() {
		try {
			const saved: Partial<Saved> = JSON.parse(localStorage.getItem(KEY) ?? '{}');
			Object.assign(this, { ...DEFAULTS, ...saved });
		} catch {
			// A corrupt entry just means defaults.
		}
	}

	save(): void {
		const saved: Saved = {
			sidebarWidth: this.sidebarWidth,
			assistantWidth: this.assistantWidth,
			split: this.split,
			orientation: this.orientation
		};
		localStorage.setItem(KEY, JSON.stringify(saved));
	}

	toggleOrientation(): void {
		this.orientation = this.orientation === 'rows' ? 'columns' : 'rows';
		this.save();
	}
}

export const layout = new LayoutState();
