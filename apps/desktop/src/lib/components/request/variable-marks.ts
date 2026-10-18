import type { Extension } from '@codemirror/state';
import {
	Decoration,
	EditorView,
	MatchDecorator,
	ViewPlugin,
	hoverTooltip,
	type DecorationSet,
	type ViewUpdate
} from '@codemirror/view';
import { mount, unmount } from 'svelte';
import type { VariableHint } from '#lib/api/index.js';
import VariableCard from '#lib/components/common/VariableCard.svelte';
import { variableAt, variablePattern } from '#lib/utils/variables.js';

const found = Decoration.mark({ class: 'rounded-sm bg-primary/10' });
const missing = Decoration.mark({
	class: 'underline decoration-destructive decoration-wavy underline-offset-[3px]'
});

/**
 * Marks each `{{variable}}` in the editor (a red wavy line when it is missing) and shows the
 * same card as the URL bar on hover. Build it again when the hints change.
 */
export function variableMarks(hints: VariableHint[], environment: string | null): Extension {
	const byName = new Map(hints.map((h) => [h.name, h]));
	const decorator = new MatchDecorator({
		regexp: variablePattern(),
		decoration: (match) => {
			const hint = byName.get(match[1].trim());
			if (!hint) return null;
			return hint.kind === 'missing' ? missing : found;
		}
	});
	const marks = ViewPlugin.fromClass(
		class {
			decorations: DecorationSet;
			constructor(view: EditorView) {
				this.decorations = decorator.createDeco(view);
			}
			update(update: ViewUpdate) {
				this.decorations = decorator.updateDeco(update, this.decorations);
			}
		},
		{ decorations: (plugin) => plugin.decorations }
	);
	const card = hoverTooltip((view, pos) => {
		const line = view.state.doc.lineAt(pos);
		const at = variableAt(line.text, pos - line.from);
		const hint = at && byName.get(at.name);
		if (!at || !hint) return null;
		return {
			pos: line.from + at.from,
			end: line.from + at.to,
			create: () => {
				const dom = document.createElement('div');
				const shown = mount(VariableCard, { target: dom, props: { hint, environment } });
				return { dom, destroy: () => void unmount(shown) };
			}
		};
	});
	// The card draws its own box, and the gap keeps the wavy line under the variable visible.
	const plain = EditorView.theme({
		'.cm-tooltip.cm-tooltip-hover': {
			border: 'none',
			backgroundColor: 'transparent',
			marginTop: '6px'
		}
	});
	return [marks, card, plain];
}
