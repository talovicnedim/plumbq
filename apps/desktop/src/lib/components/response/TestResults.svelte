<script lang="ts">
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import type { AssertionResult } from '#lib/api/index.js';
	import AskAi from '#lib/components/ai/AskAi.svelte';
	import { ASK } from '#lib/utils/ai.js';

	interface Props {
		results: AssertionResult[];
		/** For "Explain" on failed assertions. */
		requestId: string;
	}

	let { results, requestId }: Props = $props();
</script>

{#if results.length === 0}
	<p class="py-8 text-center text-sm text-muted-foreground">
		No assertions. Add them in the request's Assertions tab, e.g. <code>status == 200</code> or
		<code>body.$.id exists</code>.
	</p>
{:else}
	<ul class="grid gap-1 select-text">
		{#each results as result, i (i)}
			<li class="flex items-start gap-2 rounded-md border px-3 py-2 text-xs">
				{#if result.passed}
					<CircleCheckIcon class="mt-0.5 size-3.5 shrink-0 text-success" />
				{:else}
					<CircleXIcon class="mt-0.5 size-3.5 shrink-0 text-destructive" />
				{/if}
				<span class="grid flex-1 gap-0.5">
					<code class="font-mono">{result.expression}</code>
					<span class="text-muted-foreground">{result.message}</span>
				</span>
				{#if !result.passed}
					<AskAi
						label="Explain"
						prompt={ASK.assertion(requestId, result.expression, result.message)}
						attach={{ request: true, response: true }}
						class="-my-1 shrink-0"
					/>
				{/if}
			</li>
		{/each}
	</ul>
{/if}
