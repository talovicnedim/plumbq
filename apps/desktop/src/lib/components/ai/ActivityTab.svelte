<script lang="ts">
	import TrashIcon from '@lucide/svelte/icons/trash-2';
	import { toast } from 'svelte-sonner';
	import {
		api,
		toApiError,
		type ActivityEntry_Serialize,
		type ActivityOutcome_Serialize
	} from '#lib/api/index.js';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { confirm } from '#lib/state/confirm.svelte.js';
	import { Query } from '#lib/state/query.svelte.js';
	import { cn } from '#lib/utils.js';
	import { activitySource } from '#lib/utils/ai.js';
	import { formatTime } from '#lib/utils/format.js';

	const log = new Query<ActivityEntry_Serialize[]>(() => api.mcp.activity.list());
	const entries = $derived(log.data ?? []);
	let outcome = $state<ActivityOutcome_Serialize | 'all'>('all');
	let source = $state('all');

	const sources = $derived([...new Set(entries.map((e) => activitySource(e).label))]);
	const shown = $derived(
		entries.filter(
			(e) =>
				(outcome === 'all' || e.outcome === outcome) &&
				(source === 'all' || activitySource(e).label === source)
		)
	);

	const OUTCOME_CLASS: Record<ActivityOutcome_Serialize, string> = {
		ran: 'text-muted-foreground',
		approved: 'text-success',
		denied: 'text-destructive',
		blocked: 'text-destructive',
		failed: 'text-warning'
	};

	$effect(() => {
		void log.refresh();
	});

	async function clear() {
		const ok = await confirm({
			title: 'Clear the AI activity log?',
			description: 'The record of what the AI did is deleted from this machine.',
			confirmLabel: 'Clear',
			destructive: true
		});
		if (!ok) return;
		try {
			await api.mcp.activity.clear();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
		await log.refresh();
	}
</script>

<div class="flex h-full min-h-0 flex-col gap-3">
	<div class="flex items-center gap-2 text-sm">
		<select class="rounded-md border bg-transparent px-2 py-1 text-xs" bind:value={outcome}>
			<option value="all">All outcomes</option>
			<option value="ran">Ran</option>
			<option value="approved">Approved</option>
			<option value="denied">Denied</option>
			<option value="blocked">Blocked</option>
			<option value="failed">Failed</option>
		</select>
		<select class="rounded-md border bg-transparent px-2 py-1 text-xs" bind:value={source}>
			<option value="all">Chat and AI apps</option>
			{#each sources as name (name)}<option value={name}>{name}</option>{/each}
		</select>
		<Button size="sm" variant="ghost" class="ml-auto h-7" onclick={() => log.refresh()}
			>Refresh</Button
		>
		<Button size="sm" variant="ghost" class="h-7" onclick={clear} disabled={entries.length === 0}>
			<TrashIcon /> Clear
		</Button>
	</div>
	<QueryView query={log}>
		{#snippet children()}
			<div class="min-h-0 flex-1 overflow-auto rounded-md border">
				<table class="w-full text-xs">
					<thead class="sticky top-0 bg-muted text-left text-muted-foreground">
						<tr>
							<th class="px-2 py-1.5 font-medium">When</th>
							<th class="px-2 py-1.5 font-medium">Source</th>
							<th class="px-2 py-1.5 font-medium">Environment</th>
							<th class="px-2 py-1.5 font-medium">Outcome</th>
							<th class="px-2 py-1.5 font-medium">What</th>
						</tr>
					</thead>
					<tbody>
						{#each shown as entry, i (i)}
							{@const from = activitySource(entry)}
							<tr class="border-t align-top">
								<td class="px-2 py-1.5 whitespace-nowrap text-muted-foreground"
									>{formatTime(entry.at)}</td
								>
								<td class="px-2 py-1.5 whitespace-nowrap" title={entry.actor}>
									<span
										class={cn(
											'rounded px-1.5 py-0.5 text-[10px] font-medium',
											from.chat ? 'bg-primary/10 text-primary' : 'bg-muted text-foreground'
										)}
									>
										{from.label}
									</span>
								</td>
								<td class="px-2 py-1.5 whitespace-nowrap text-muted-foreground">
									{entry.environment ?? '-'}
								</td>
								<td
									class={cn(
										'px-2 py-1.5 whitespace-nowrap capitalize',
										OUTCOME_CLASS[entry.outcome]
									)}
								>
									{entry.outcome}
								</td>
								<td class="px-2 py-1.5 wrap-anywhere select-text">
									<span class="font-mono text-muted-foreground">{entry.tool}</span>
									{entry.summary}
								</td>
							</tr>
						{:else}
							<tr>
								<td colspan="5" class="p-6 text-center text-muted-foreground">
									Nothing yet. Every call from the chat and AI apps shows up here.
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/snippet}
	</QueryView>
</div>
