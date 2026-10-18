import { Prec, type Extension } from '@codemirror/state';
import {
	Decoration,
	EditorView,
	ViewPlugin,
	keymap,
	type DecorationSet,
	type ViewUpdate
} from '@codemirror/view';
import type { MatchRange } from '#lib/utils/body-search.js';

const match = Decoration.mark({ class: 'rounded-sm bg-warning/25' });
const currentMatch = Decoration.mark({ class: 'rounded-sm bg-warning/60 ring-1 ring-warning' });

/**
 * Highlights the matches, marks the current one and scrolls it into view. ⌘F focuses the
 * search box instead of opening the editor's own search panel. Build it again when the
 * matches or the current one change.
 */
export function bodySearch(ranges: MatchRange[], current: number, onfind: () => void): Extension {
	const plugin = ViewPlugin.fromClass(
		class {
			decorations: DecorationSet;
			constructor(view: EditorView) {
				this.decorations = highlight(view, ranges, current);
				scrollToMatch(view, ranges[current]);
			}
			update(update: ViewUpdate) {
				if (update.docChanged) this.decorations = highlight(update.view, ranges, current);
			}
		},
		{ decorations: (instance) => instance.decorations }
	);
	const find = Prec.highest(
		keymap.of([
			{
				key: 'Mod-f',
				run: () => {
					onfind();
					return true;
				}
			}
		])
	);
	return [plugin, find];
}

function highlight(view: EditorView, ranges: MatchRange[], current: number): DecorationSet {
	const length = view.state.doc.length;
	const marks = ranges
		.map((range, i) => ({ range, mark: i === current ? currentMatch : match }))
		.filter(({ range }) => range.to <= length)
		.map(({ range, mark }) => mark.range(range.from, range.to));
	return Decoration.set(marks);
}

// Done after the editor's own update, which may still be replacing the document.
function scrollToMatch(view: EditorView, range: MatchRange | undefined) {
	if (!range) return;
	view.requestMeasure({
		key: 'body-search-scroll',
		read: () => range.to <= view.state.doc.length && !isVisible(view, range.from),
		write: (hidden) => {
			if (hidden)
				view.dispatch({ effects: EditorView.scrollIntoView(range.from, { y: 'center' }) });
		}
	});
}

function isVisible(view: EditorView, pos: number): boolean {
	const box = view.coordsAtPos(pos);
	if (!box) return false;
	const area = view.scrollDOM.getBoundingClientRect();
	return box.top >= area.top && box.bottom <= area.bottom;
}
