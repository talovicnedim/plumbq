<script lang="ts">
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import CircleMinusIcon from '@lucide/svelte/icons/circle-minus';
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import type { RunOutcome } from '#lib/api/index.js';
	import { runner } from '#lib/state/runner.svelte.js';
	import { cn } from '#lib/utils.js';
	import { formatDuration } from '#lib/utils/format.js';
	import { methodColor, statusColor } from '#lib/utils/http.js';

	interface Props {
		outcome: RunOutcome;
		showIteration: boolean;
	}

	let { outcome, showIteration }: Props = $props();

	const failedTests = $derived(outcome.assertions.filter((a) => !a.passed));
</script>

<tr
	class="cursor-pointer border-t align-top hover:bg-muted/50"
	title="Open with this response"
	onclick={() => runner.showOutcome(outcome)}
>
	<td class="px-3 py-2">
		{#if outcome.state === 'passed'}
			<CircleCheckIcon class="size-4 text-success" />
		{:else if outcome.state === 'failed'}
			<CircleXIcon class="size-4 text-destructive" />
		{:else}
			<CircleMinusIcon class="size-4 text-muted-foreground" />
		{/if}
	</td>
	{#if showIteration}<td class="px-2 py-2 text-xs text-muted-foreground">{outcome.iteration}</td
		>{/if}
	<td class="px-2 py-2">
		<span class={cn('mr-2 font-mono text-[11px] font-semibold', methodColor(outcome.method))}>
			{outcome.method}
		</span>
		{outcome.name}
		{#if outcome.error}
			<p class="mt-0.5 text-xs break-all text-destructive">{outcome.error}</p>
		{:else if outcome.skipped}
			<p class="mt-0.5 text-xs text-muted-foreground">{outcome.skipped}</p>
		{/if}
		{#each failedTests as assertion, j (j)}
			<p class="mt-0.5 font-mono text-xs text-destructive">
				✗ {assertion.expression}: {assertion.message}
			</p>
		{/each}
	</td>
	<td class={cn('px-2 py-2 font-mono text-xs', outcome.status && statusColor(outcome.status))}>
		{outcome.status ?? '-'}
	</td>
	<td class="px-2 py-2 text-xs text-muted-foreground tabular-nums">
		{outcome.status ? formatDuration(outcome.durationMs) : '-'}
	</td>
	<td class="px-2 py-2 text-xs tabular-nums">
		{#if outcome.assertions.length}
			{outcome.assertions.length - failedTests.length}/{outcome.assertions.length}
		{:else}
			<span class="text-muted-foreground">-</span>
		{/if}
	</td>
</tr>
