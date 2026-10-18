<script lang="ts">
	import HistoryIcon from '@lucide/svelte/icons/history';
	import { toast } from 'svelte-sonner';
	import { api, toApiError } from '#lib/api/index.js';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { Query } from '#lib/state/query.svelte.js';
	import type { RequestTab } from '#lib/state/tabs.svelte.js';
	import { cn } from '#lib/utils.js';
	import { formatDuration, formatTime } from '#lib/utils/format.js';
	import { statusColor } from '#lib/utils/http.js';

	interface Props {
		tab: RequestTab;
	}

	let { tab }: Props = $props();

	const history = new Query(() => api.history.list(tab.id));

	async function show(id: string) {
		try {
			tab.entry = await api.history.get(tab.id, id);
			tab.error = undefined;
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}
</script>

<DropdownMenu.Root onOpenChange={(open) => open && void history.refresh()}>
	<DropdownMenu.Trigger
		class="flex items-center gap-1.5 rounded-md px-2 py-1 text-muted-foreground hover:bg-muted hover:text-foreground"
	>
		<HistoryIcon class="size-3.5" /> History
	</DropdownMenu.Trigger>
	<!-- pr-3: macOS draws the scrollbar over the right edge, which would cover the durations. -->
	<DropdownMenu.Content align="end" class="max-h-80 w-84 overflow-y-auto pr-3">
		<DropdownMenu.Label class="text-xs text-muted-foreground"
			>Kept on this machine, newest first</DropdownMenu.Label
		>
		<QueryView query={history} empty="Nothing sent yet." isEmpty={(rows) => rows.length === 0}>
			{#snippet children(rows)}
				{#each rows as row (row.id)}
					<DropdownMenu.Item
						onclick={() => show(row.id)}
						class={cn('gap-3 font-mono text-xs', tab.entry?.id === row.id && 'bg-muted')}
					>
						<span class={cn('font-semibold', statusColor(row.status))}>{row.status}</span>
						<span class="text-muted-foreground">{formatTime(row.sentAt)}</span>
						{#if row.passed + row.failed > 0}
							<span class={row.failed ? 'text-destructive' : 'text-muted-foreground'}>
								{row.passed}/{row.passed + row.failed}
							</span>
						{/if}
						{#if row.environment}<span class="min-w-0 truncate text-muted-foreground"
								>{row.environment}</span
							>{/if}
						<span class="ml-auto shrink-0 text-muted-foreground tabular-nums"
							>{formatDuration(row.durationMs)}</span
						>
					</DropdownMenu.Item>
				{/each}
			{/snippet}
		</QueryView>
	</DropdownMenu.Content>
</DropdownMenu.Root>
