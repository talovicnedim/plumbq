<script lang="ts">
	import type { PaletteEntry } from '#lib/components/palette/entries.js';
	import { cn } from '#lib/utils.js';
	import { methodColor, statusColor } from '#lib/utils/http.js';

	interface Props {
		entry: PaletteEntry;
		/** Searching everything: say what kind of thing each row is. */
		showKind: boolean;
	}

	let { entry, showKind }: Props = $props();
</script>

<span class="flex w-12 shrink-0 items-center">
	{#if entry.method}
		<span class={cn('truncate font-mono text-[11px] font-semibold', methodColor(entry.method))}>
			{entry.method}
		</span>
	{:else}
		<span
			class="flex size-7 items-center justify-center rounded-md bg-muted/70 group-data-selected/command-item:bg-background"
		>
			{#if entry.dot}
				<span class={cn('size-2 rounded-full', entry.dot)}></span>
			{:else if entry.icon}
				<entry.icon class="size-3.5 text-muted-foreground" />
			{/if}
		</span>
	{/if}
</span>
<span class="flex min-w-0 flex-1 items-baseline gap-2">
	<span class="max-w-full shrink-0 truncate text-[13px] text-foreground">{entry.title}</span>
	{#if entry.subtitle}
		<span class="min-w-0 truncate text-xs text-muted-foreground">{entry.subtitle}</span>
	{/if}
</span>
<span class="flex shrink-0 items-center gap-2 text-[11px] text-muted-foreground">
	{#if entry.status}
		<span class={cn('font-mono', statusColor(entry.status))}>{entry.status}</span>
	{/if}
	{#if entry.meta}<span>{entry.meta}</span>{/if}
	{#if entry.shortcut}
		<kbd class="rounded border bg-muted px-1.5 py-0.5 font-sans text-[10px]">{entry.shortcut}</kbd>
	{/if}
	{#if showKind}<span class="w-18 text-right opacity-70">{entry.kind}</span>{/if}
</span>
