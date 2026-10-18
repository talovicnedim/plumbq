<script lang="ts">
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import WrenchIcon from '@lucide/svelte/icons/wrench';
	import { cn } from '#lib/utils.js';
	import { toolLabel, type ChatItem } from '#lib/utils/chat.js';

	interface Props {
		item: Extract<ChatItem, { kind: 'tool' }>;
	}

	let { item }: Props = $props();

	let expanded = $state(false);
</script>

<div
	class={cn(
		'rounded-md border text-xs',
		item.status === 'error' && 'border-destructive/40 bg-destructive/5'
	)}
>
	<button
		type="button"
		class="flex w-full items-center gap-2 px-2.5 py-1.5 text-left"
		aria-expanded={expanded}
		onclick={() => (expanded = !expanded)}
		disabled={!item.excerpt}
	>
		{#if item.status === 'running'}
			<LoaderIcon class="size-3.5 shrink-0 animate-spin text-muted-foreground" />
		{:else}
			<WrenchIcon
				class={cn('size-3.5 shrink-0', item.status === 'ok' ? 'text-success' : 'text-destructive')}
			/>
		{/if}
		<span class="font-medium">{toolLabel(item.name)}</span>
		{#if item.summary}<span class="truncate font-mono text-muted-foreground">{item.summary}</span
			>{/if}
		{#if item.status === 'error'}<span class="ml-auto shrink-0 text-destructive">not done</span
			>{/if}
		{#if item.excerpt}
			<ChevronRightIcon
				class={cn(
					'size-3.5 shrink-0 text-muted-foreground transition-transform',
					expanded && 'rotate-90',
					item.status !== 'error' && 'ml-auto'
				)}
			/>
		{/if}
	</button>
	{#if expanded && item.excerpt}
		<pre
			class="max-h-48 overflow-auto border-t px-2.5 py-1.5 font-mono text-[11px] whitespace-pre-wrap text-muted-foreground select-text">{item.excerpt}</pre>
	{/if}
</div>
