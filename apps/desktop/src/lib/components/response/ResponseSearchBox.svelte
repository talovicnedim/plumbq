<script lang="ts">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import ChevronUpIcon from '@lucide/svelte/icons/chevron-up';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { Button } from '#lib/components/ui/button/index.js';

	interface Props {
		value: string;
		placeholder: string;
		/** A match counter such as "2 of 5". Empty shows nothing. */
		status?: string;
		/** A filter error, shown instead of the counter. */
		error?: string;
		/** Shows the previous and next buttons. */
		canStep?: boolean;
		onstep: (direction: 1 | -1) => void;
	}

	let {
		value = $bindable(),
		placeholder,
		status,
		error,
		canStep = false,
		onstep
	}: Props = $props();

	let input = $state<HTMLInputElement | null>(null);

	export function focus() {
		input?.focus();
		input?.select();
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			onstep(e.shiftKey ? -1 : 1);
		} else if (e.key === 'Escape' && value) {
			e.preventDefault();
			value = '';
		}
	}
</script>

<label
	class="flex h-7 shrink-0 items-center gap-2 rounded-md border px-2 text-xs focus-within:ring-1 focus-within:ring-ring"
>
	<SearchIcon class="size-3.5 shrink-0 text-muted-foreground" />
	<input
		bind:this={input}
		class="min-w-0 flex-1 bg-transparent font-mono outline-none placeholder:text-muted-foreground"
		{placeholder}
		bind:value
		spellcheck="false"
		{onkeydown}
	/>
	{#if error}
		<span class="shrink-0 text-destructive">{error}</span>
	{:else if status}
		<span class="shrink-0 text-muted-foreground tabular-nums">{status}</span>
	{/if}
	{#if canStep}
		<span class="-mr-1 flex shrink-0">
			<Button
				variant="ghost"
				size="icon-xs"
				class="size-5"
				aria-label="Previous match"
				onclick={() => onstep(-1)}
			>
				<ChevronUpIcon />
			</Button>
			<Button
				variant="ghost"
				size="icon-xs"
				class="size-5"
				aria-label="Next match"
				onclick={() => onstep(1)}
			>
				<ChevronDownIcon />
			</Button>
		</span>
	{/if}
</label>
