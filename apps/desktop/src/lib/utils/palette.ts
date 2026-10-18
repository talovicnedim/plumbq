/** What the palette searches, chosen by a prefix: `>` commands, `#` tags, `@` environments. */
type PaletteMode = 'all' | 'commands' | 'tags' | 'environments';

export function parseQuery(text: string): { mode: PaletteMode; query: string } {
	const prefixes: [string, PaletteMode][] = [
		['>', 'commands'],
		['#', 'tags'],
		['@', 'environments']
	];
	for (const [prefix, mode] of prefixes) {
		if (text.startsWith(prefix)) return { mode, query: text.slice(1).trim() };
	}
	return { mode: 'all', query: text.trim() };
}

export interface Searchable {
	title: string;
	subtitle?: string;
	/** Extra words that should find it (method, URL, ids, synonyms). */
	keywords?: string;
	tags?: string[];
}

/**
 * How well `query` matches; 0 means no match. Every space-separated term has to match. Per
 * term: the title starting with it scores highest, then a word in the title starting with it,
 * then anywhere in the title, then the subtitle or keywords, then the title's letters in order
 * (`nreq` finds "New request"; not in keywords, where long URLs would match almost anything).
 * `tag:smoke` matches only items tagged exactly `smoke`.
 */
export function score(item: Searchable, query: string): number {
	const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
	let total = 0;
	for (const term of terms) {
		const points = term.startsWith('tag:') ? tagScore(item, term.slice(4)) : termScore(item, term);
		if (points === 0) return 0;
		total += points;
	}
	return terms.length ? total : 1;
}

function termScore(item: Searchable, term: string): number {
	const title = item.title.toLowerCase();
	if (title.startsWith(term)) return 100;
	if (title.split(/[\s/:._-]+/).some((word) => word.startsWith(term))) return 80;
	if (title.includes(term)) return 60;
	const rest = `${item.subtitle ?? ''} ${item.keywords ?? ''}`.toLowerCase();
	if (rest.split(/[\s/:._-]+/).some((word) => word.startsWith(term))) return 40;
	if (rest.includes(term)) return 30;
	return inOrder(title, term) ? 20 : 0;
}

function tagScore(item: Searchable, tag: string): number {
	return item.tags?.some((t) => t.toLowerCase() === tag) ? 50 : 0;
}

function inOrder(text: string, query: string): boolean {
	let at = 0;
	for (const char of query) {
		at = text.indexOf(char, at);
		if (at === -1) return false;
		at++;
	}
	return true;
}

/** Matching items, best first; ties keep their original order (which carries priority). */
export function rank<T extends Searchable>(items: T[], query: string, limit = 50): T[] {
	return items
		.map((item, index) => ({ item, index, score: score(item, query) }))
		.filter((entry) => entry.score > 0)
		.sort((a, b) => b.score - a.score || a.index - b.index)
		.slice(0, limit)
		.map((entry) => entry.item);
}

/** Items with a tag starting with `prefix` (all tagged items for ``), exact tags first. */
export function byTag<T extends Searchable>(items: T[], prefix: string): T[] {
	const p = prefix.toLowerCase();
	const points = (item: T) => {
		const tags = (item.tags ?? []).map((t) => t.toLowerCase());
		if (p && tags.includes(p)) return 2;
		return tags.some((t) => t.startsWith(p)) ? 1 : 0;
	};
	return items
		.map((item, index) => ({ item, index, points: points(item) }))
		.filter((entry) => entry.points > 0)
		.sort((a, b) => b.points - a.points || a.index - b.index)
		.map((entry) => entry.item);
}

/** `items` with the ids in `first` moved to the front, in that order. */
export function recentFirst<T extends { id: string }>(items: T[], first: string[]): T[] {
	const order = new Map(first.map((id, i) => [id, i]));
	const position = (item: T) => order.get(item.id) ?? first.length;
	return items
		.map((item, index) => ({ item, index }))
		.sort((a, b) => position(a.item) - position(b.item) || a.index - b.index)
		.map((entry) => entry.item);
}
