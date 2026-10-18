<script lang="ts">
	import type { BodyComparison } from '#lib/api/index.js';
	import { cn } from '#lib/utils.js';
	import { formatBodyChange } from '#lib/utils/format.js';

	interface Props {
		comparison: BodyComparison;
	}

	let { comparison }: Props = $props();
</script>

<div
	class="max-h-36 shrink-0 overflow-y-auto rounded-md border bg-muted/30 p-2 font-mono text-xs select-text"
>
	{#each comparison.changes as change, i (i)}
		<p
			class={cn(
				change.kind === 'added' && 'text-success',
				change.kind === 'removed' && 'text-danger'
			)}
		>
			{formatBodyChange(change)}
		</p>
	{:else}
		<p class="font-sans text-muted-foreground">Bodies are identical.</p>
	{/each}
	{#if comparison.omitted > 0}<p class="text-muted-foreground">
			… and {comparison.omitted} more
		</p>{/if}
</div>
