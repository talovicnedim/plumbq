import { api } from '#lib/api/index.js';
import { confirm } from './confirm.svelte.js';
import { tabs } from './tabs.svelte.js';

/** Asks before discarding unsaved tabs. Returns whether it's fine to go on. */
export async function confirmDiscardAll(action: string): Promise<boolean> {
	const dirty = tabs.all.filter((t) => tabs.isDirty(t));
	if (dirty.length === 0) return true;
	return confirm({
		title: `${action} with unsaved changes?`,
		description:
			dirty.length === 1
				? `"${dirty[0].draft.name || dirty[0].id}" has unsaved changes. They will be lost.`
				: `${dirty.length} requests have unsaved changes. They will be lost.`,
		confirmLabel: `Discard and ${action.toLowerCase()}`,
		destructive: true
	});
}

/** Quits the app, after asking when tabs have unsaved changes. */
export async function quitApp(): Promise<void> {
	if (await confirmDiscardAll('Quit')) await api.app.quit();
}
