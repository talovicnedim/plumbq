import { commandEntries } from '#lib/components/palette/command-entries.js';
import {
	environmentEntries,
	quickEntries,
	recentlySent,
	requestEntries,
	workspaceEntries,
	type PaletteEntry,
	type PaletteHooks
} from '#lib/components/palette/entries.js';
import { byTag, parseQuery, rank } from '#lib/utils/palette.js';

interface PaletteSection {
	title: string;
	items: PaletteEntry[];
}

/** What the palette lists for `search`: a prefix picks one kind, no text the start page. */
export function paletteSections(search: string, hooks: PaletteHooks): PaletteSection[] {
	const { mode, query } = parseQuery(search);
	if (mode === 'commands')
		return [{ title: 'Commands', items: rank(commandEntries(hooks), query) }];
	if (mode === 'environments') {
		return [{ title: 'Environments', items: rank(environmentEntries(), query) }];
	}
	const requests = requestEntries();
	if (mode === 'tags') {
		const [tag = '', ...rest] = query.split(/\s+/);
		return [{ title: 'Tagged requests', items: rank(byTag(requests, tag), rest.join(' ')) }];
	}
	if (!query) {
		return [
			{ title: 'Recently sent', items: recentlySent(requests) },
			{ title: 'Actions', items: quickEntries(hooks) }
		];
	}
	const everything = [
		...requests,
		...commandEntries(hooks),
		...environmentEntries(),
		...workspaceEntries()
	];
	return [{ title: 'Results', items: rank(everything, query) }];
}
