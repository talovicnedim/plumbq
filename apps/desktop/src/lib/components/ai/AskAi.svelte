<script lang="ts">
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import { ai } from '#lib/state/ai.svelte.js';
	import { assistant, type ContextChips } from '#lib/state/assistant.svelte.js';
	import { cn } from '#lib/utils.js';

	interface Props {
		/** The verb on the button: "Explain this", "Why did this fail?". */
		label: string;
		/** What the chat is asked. */
		prompt: string;
		/** What goes along with the question (the request, its last response...). */
		attach?: Partial<ContextChips>;
		class?: string;
	}

	let { label, prompt, attach, class: className }: Props = $props();
</script>

{#if !ai.off}
	<button
		type="button"
		class={cn(
			'inline-flex h-6 items-center gap-1 rounded-md px-1.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground',
			className
		)}
		title="Ask the AI chat"
		onclick={() => assistant.explain(prompt, attach)}
	>
		<SparklesIcon class="size-3 text-primary" />
		{label}
	</button>
{/if}
