<script lang="ts">
	import CheckIcon from '@lucide/svelte/icons/check';
	import CopyIcon from '@lucide/svelte/icons/copy';

	interface Props {
		value: string;
		/** Monospace block for multi-line snippets; one line otherwise. */
		multiline?: boolean;
		label?: string;
	}

	let { value, multiline = false, label = 'Copy' }: Props = $props();

	let copied = $state(false);

	async function copy() {
		await navigator.clipboard.writeText(value);
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}
</script>

<div class="relative">
	{#if multiline}
		<pre
			class="max-h-56 overflow-auto rounded-lg border bg-muted/40 p-3 pr-10 font-mono text-xs whitespace-pre-wrap select-text">{value}</pre>
	{:else}
		<div
			class="truncate rounded-md border bg-muted/40 py-1.5 pr-10 pl-2.5 font-mono text-xs select-text"
			title={value}
		>
			{value}
		</div>
	{/if}
	<button
		type="button"
		class="absolute top-1 right-1 rounded-md p-1.5 text-muted-foreground hover:bg-background hover:text-foreground"
		title={label}
		aria-label={label}
		onclick={copy}
	>
		{#if copied}<CheckIcon class="size-3.5" />{:else}<CopyIcon class="size-3.5" />{/if}
	</button>
</div>
