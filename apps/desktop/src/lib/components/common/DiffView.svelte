<script lang="ts">
	import { json } from '@codemirror/lang-json';
	import { syntaxHighlighting } from '@codemirror/language';
	import { MergeView } from '@codemirror/merge';
	import { EditorState } from '@codemirror/state';
	import { EditorView } from '@codemirror/view';
	import { classHighlighter } from '@lezer/highlight';
	import { basicSetup } from 'codemirror';

	interface Props {
		before: string;
		after: string;
		json?: boolean;
		class?: string;
	}

	let { before, after, json: isJson = false, class: className }: Props = $props();

	let host = $state<HTMLDivElement | null>(null);

	// Rebuilt whenever an input changes; a read-only diff has no state worth keeping.
	$effect(() => {
		if (!host) return;
		const extensions = [
			basicSetup,
			syntaxHighlighting(classHighlighter),
			isJson ? json() : [],
			EditorView.lineWrapping,
			EditorState.readOnly.of(true),
			EditorView.editable.of(false)
		];
		const view = new MergeView({
			parent: host,
			a: { doc: before, extensions },
			b: { doc: after, extensions },
			collapseUnchanged: { margin: 3, minSize: 6 },
			gutter: true
		});
		return () => view.destroy();
	});
</script>

<div
	bind:this={host}
	class={['min-h-0 overflow-auto rounded-md border text-[13px] select-text', className]}
></div>
