<script lang="ts">
	import TrashIcon from '@lucide/svelte/icons/trash-2';
	import type { HistorySummary, ResponseExample } from '#lib/api/index.js';
	import { cn } from '#lib/utils.js';
	import { formatTime } from '#lib/utils/format.js';
	import { statusColor } from '#lib/utils/http.js';

	interface Props {
		examples: ResponseExample[];
		/** Earlier sends, without the one shown now. */
		history: HistorySummary[];
		/** `example:<name>` or `history:<id>`. */
		selected: string | undefined;
		onremove: (name: string) => void;
	}

	let { examples, history, selected = $bindable(), onremove }: Props = $props();
</script>

<div class="flex flex-col gap-3 text-sm">
	<div class="grid gap-0.5">
		<h3 class="px-2 text-xs font-medium text-muted-foreground">Saved examples</h3>
		{#each examples as ex (ex.name)}
			<div
				class={cn(
					'group flex items-center rounded-md hover:bg-muted',
					selected === `example:${ex.name}` && 'bg-muted'
				)}
			>
				<button
					type="button"
					class="flex min-w-0 flex-1 items-center gap-2 px-2 py-1.5 text-left"
					onclick={() => (selected = `example:${ex.name}`)}
				>
					<span class={cn('font-mono text-xs', statusColor(ex.status))}>{ex.status}</span>
					<span class="truncate">{ex.name}</span>
				</button>
				<button
					type="button"
					class="mr-1 p-1 text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-destructive"
					aria-label="Delete example"
					onclick={() => onremove(ex.name)}
				>
					<TrashIcon class="size-3.5" />
				</button>
			</div>
		{:else}
			<p class="px-2 text-xs text-muted-foreground">
				None yet. Use "Save as example" on a response.
			</p>
		{/each}
	</div>
	<div class="grid gap-0.5">
		<h3 class="px-2 text-xs font-medium text-muted-foreground">History</h3>
		{#each history as row (row.id)}
			<button
				type="button"
				class={cn(
					'flex items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-muted',
					selected === `history:${row.id}` && 'bg-muted'
				)}
				onclick={() => (selected = `history:${row.id}`)}
			>
				<span class={cn('font-mono text-xs', statusColor(row.status))}>{row.status}</span>
				<span class="text-xs">{formatTime(row.sentAt)}</span>
				<span class="ml-auto truncate text-xs text-muted-foreground">{row.environment ?? ''}</span>
			</button>
		{:else}
			<p class="px-2 text-xs text-muted-foreground">No earlier sends.</p>
		{/each}
	</div>
</div>
