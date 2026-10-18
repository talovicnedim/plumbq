<script lang="ts">
	import type { VariableHint } from '#lib/api/index.js';
	import { cn } from '#lib/utils.js';
	import { describeVariable } from '#lib/utils/variables.js';

	interface Props {
		hint: VariableHint;
		/** The selected environment, named as the source of plain values. */
		environment: string | null;
	}

	let { hint, environment }: Props = $props();

	const info = $derived(describeVariable(hint, environment));
</script>

<div
	class="w-max max-w-80 rounded-md border bg-popover px-2.5 py-2 font-sans text-xs text-popover-foreground shadow-md"
>
	<div class="flex items-baseline justify-between gap-4">
		<span
			class={cn(
				'min-w-0 font-mono font-medium wrap-anywhere',
				info.missing ? 'text-destructive' : 'text-primary'
			)}
		>
			{hint.name}
		</span>
		<span class={cn('shrink-0', info.missing ? 'text-destructive' : 'text-muted-foreground')}>
			{info.source}
		</span>
	</div>
	{#if info.value}
		<p class="mt-1 line-clamp-3 font-mono wrap-anywhere">{info.value}</p>
	{/if}
</div>
