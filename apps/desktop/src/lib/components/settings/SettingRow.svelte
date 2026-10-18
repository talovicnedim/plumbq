<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		label: string;
		hint?: string;
		children: Snippet;
	}

	let { label, hint, children }: Props = $props();
</script>

<div class="flex items-center justify-between gap-6 py-2.5 not-first:border-t">
	<div class="grid gap-0.5">
		<span class="text-sm font-medium">{label}</span>
		{#if hint}<span class="text-xs text-muted-foreground">{hint}</span>{/if}
	</div>
	<div class="shrink-0">{@render children()}</div>
</div>
