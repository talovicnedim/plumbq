/** Matches counted and highlighted at most, so typing stays instant on a huge body. */
export const MAX_MATCHES = 1000;

export type BodySearch =
	{ kind: 'none' } | { kind: 'find'; query: string } | { kind: 'path'; path: string };

export interface MatchRange {
	from: number;
	to: number;
}

export interface Matches {
	ranges: MatchRange[];
	/** There were more matches than `MAX_MATCHES`. */
	capped: boolean;
}

/** Text starting with `$` filters a JSON body by path; anything else is a find. */
export function searchMode(input: string, isJson: boolean): BodySearch {
	const text = input.trim();
	if (!text) return { kind: 'none' };
	if (isJson && text.startsWith('$')) return { kind: 'path', path: text };
	return { kind: 'find', query: text };
}

/** Case-insensitive, plain-text matches in document order. */
export function findMatches(text: string, query: string, limit = MAX_MATCHES): Matches {
	const ranges: MatchRange[] = [];
	const pattern = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi');
	for (const match of text.matchAll(pattern)) {
		if (ranges.length === limit) return { ranges, capped: true };
		ranges.push({ from: match.index, to: match.index + match[0].length });
	}
	return { ranges, capped: false };
}

/** The next (1) or previous (-1) match, wrapping around at both ends. */
export function stepIndex(current: number, count: number, direction: 1 | -1): number {
	if (count === 0) return 0;
	return (current + direction + count) % count;
}

export function countLabel(index: number, count: number, capped: boolean): string {
	if (count === 0) return 'No matches';
	return `${index + 1} of ${count}${capped ? '+' : ''}`;
}
