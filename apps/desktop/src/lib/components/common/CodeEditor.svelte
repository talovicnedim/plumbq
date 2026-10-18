<script lang="ts">
	import { json } from '@codemirror/lang-json';
	import { indentUnit, syntaxHighlighting } from '@codemirror/language';
	import { Compartment, EditorState, Prec, type Extension } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { classHighlighter } from '@lezer/highlight';
	import { basicSetup } from 'codemirror';
	import { untrack } from 'svelte';
	import { settings } from '#lib/state/settings.svelte.js';

	interface Props {
		value: string;
		/** Enables JSON highlighting and folding. */
		json?: boolean;
		readonly?: boolean;
		/** Wrap long lines; defaults to the word-wrap setting. */
		wrap?: boolean;
		/** Extra behavior that changes without recreating the editor, e.g. variable marks. */
		extensions?: Extension;
		class?: string;
	}

	let {
		value = $bindable(),
		json: isJson = false,
		readonly = false,
		wrap,
		extensions = [],
		class: className
	}: Props = $props();

	const lineWrap = $derived(wrap ?? settings.values.wordWrap);
	const tabSize = $derived(settings.values.tabSize);

	let host = $state<HTMLDivElement | null>(null);

	/** Pasting JSON into an empty JSON editor pretty-prints it, when that setting is on. */
	const pasteFormatting = EditorView.domEventHandlers({
		paste(event, view) {
			if (!isJson || readonly || !settings.values.formatJsonOnPaste) return false;
			if (view.state.doc.length > 0) return false;
			const text = event.clipboardData?.getData('text/plain') ?? '';
			try {
				const pretty = JSON.stringify(JSON.parse(text), null, settings.values.tabSize);
				view.dispatch({ changes: { from: 0, insert: pretty } });
				event.preventDefault();
				return true;
			} catch {
				return false;
			}
		}
	});
	let view: EditorView | undefined;
	const extra = new Compartment();

	// Create the editor once per language; it owns its document from then on. Reading `value`
	// untracked keeps typing from recreating the editor.
	$effect(() => {
		if (!host) return;
		const language = isJson ? [json()] : [];
		const wrapping = lineWrap ? [EditorView.lineWrapping] : [];
		const indent = [EditorState.tabSize.of(tabSize), indentUnit.of(' '.repeat(tabSize))];
		view = new EditorView({
			parent: host,
			state: EditorState.create({
				doc: untrack(() => value),
				extensions: [
					// App shortcuts (⌘↵ send, ⌘S save) win over the editor's own bindings, which would
					// otherwise insert a blank line first. Returning true only stops the editor; the key
					// event still reaches the app's handler.
					Prec.highest(
						keymap.of([
							{ key: 'Mod-Enter', run: () => true },
							{ key: 'Mod-s', run: () => true }
						])
					),
					basicSetup,
					syntaxHighlighting(classHighlighter),
					language,
					wrapping,
					indent,
					pasteFormatting,
					extra.of(untrack(() => extensions)),
					EditorState.readOnly.of(untrack(() => readonly)),
					EditorView.theme({ '&': { height: '100%' }, '.cm-scroller': { overflow: 'auto' } }),
					EditorView.updateListener.of((update) => {
						if (update.docChanged) value = update.state.doc.toString();
					})
				]
			})
		});
		return () => view?.destroy();
	});

	$effect(() => {
		const next = extensions;
		view?.dispatch({ effects: extra.reconfigure(next) });
	});

	// Outside changes (switching tabs, a new response) replace the document.
	$effect(() => {
		const next = value;
		if (view && view.state.doc.toString() !== next) {
			view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
		}
	});
</script>

<div
	bind:this={host}
	class={['min-h-0 overflow-hidden rounded-md border select-text', className]}
	style="font-size: var(--editor-font-size, 13px)"
></div>
