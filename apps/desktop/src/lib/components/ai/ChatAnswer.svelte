<script lang="ts">
	import CopyField from '#lib/components/common/CopyField.svelte';
	import { renderMarkdown, splitCodeBlocks } from '#lib/utils/markdown.js';

	interface Props {
		/** The model's answer as markdown; it grows while the answer streams in. */
		text: string;
	}

	let { text }: Props = $props();

	const parts = $derived(splitCodeBlocks(text));
</script>

<div class="grid gap-2">
	{#each parts as part, i (i)}
		{#if part.kind === 'code'}
			<CopyField value={part.text} multiline label="Copy code" />
		{:else}
			<!-- renderMarkdown escapes all HTML in model output before adding its own tags. -->
			<div
				class="text-sm leading-relaxed select-text [&_a]:text-primary [&_a]:underline [&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:font-mono [&_code]:text-xs [&_h1]:mb-2 [&_h1]:font-semibold [&_h2]:mb-2 [&_h2]:font-semibold [&_h3]:mb-1 [&_h3]:font-medium [&_ol]:mb-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:mb-2 [&_p:last-child]:mb-0 [&_ul]:mb-2 [&_ul]:list-disc [&_ul]:pl-5"
			>
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				{@html renderMarkdown(part.text)}
			</div>
		{/if}
	{/each}
</div>
