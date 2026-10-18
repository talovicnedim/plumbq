<script lang="ts">
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import AskAi from '#lib/components/ai/AskAi.svelte';
	import type { RequestTab } from '#lib/state/tabs.svelte.js';
	import { cn } from '#lib/utils.js';
	import { ASK } from '#lib/utils/ai.js';
	import { summarize } from '#lib/utils/assertions.js';
	import { formatBytes, formatDuration } from '#lib/utils/format.js';
	import { statusColor } from '#lib/utils/http.js';
	import { headersSize } from '#lib/utils/preview.js';
	import HistoryMenu from './HistoryMenu.svelte';

	interface Props {
		tab: RequestTab;
	}

	let { tab }: Props = $props();

	let now = $state(Date.now());

	const response = $derived(tab.entry?.response);
	const tests = $derived(summarize(response?.assertions));

	// Ticks only while a request is in flight.
	$effect(() => {
		if (!tab.sending) return;
		const timer = setInterval(() => (now = Date.now()), 100);
		return () => clearInterval(timer);
	});
</script>

<div class="flex h-11 shrink-0 items-center gap-3 border-b px-4 text-xs">
	{#if tab.sending}
		<span class="flex items-center gap-2 text-muted-foreground">
			<LoaderIcon class="size-3.5 animate-spin" /> Sending…
			<span class="tabular-nums">{formatDuration(now - (tab.startedAt ?? now))}</span>
		</span>
	{:else if response}
		<span
			class={cn(
				'rounded-md bg-current/10 px-2 py-0.5 font-mono text-[13px] font-semibold',
				statusColor(response.status)
			)}
		>
			{response.status}
			{response.statusText}
		</span>
		<span class="text-muted-foreground tabular-nums">{formatDuration(response.durationMs)}</span>
		<span
			class="text-muted-foreground tabular-nums"
			title={`Body ${formatBytes(response.size)}, headers ${formatBytes(headersSize(response.headers))}`}
		>
			{formatBytes(response.size + headersSize(response.headers))}{response.truncated
				? ' (body truncated)'
				: ''}
		</span>
		{#if tests.total > 0}
			<span
				class={cn(
					'rounded-md px-2 py-0.5 font-medium',
					tests.allPassed ? 'bg-success/10 text-success' : 'bg-destructive/10 text-destructive'
				)}
			>
				{tests.label}
			</span>
		{/if}
	{:else}
		<span class="font-medium text-muted-foreground">Response</span>
	{/if}
	{#if tab.cancelled && !tab.sending}
		<span class="rounded-md bg-muted px-2 py-0.5 text-muted-foreground">Cancelled</span>
	{/if}
	<div class="ml-auto flex items-center gap-1">
		{#if response && !tab.sending && !tab.error}
			<AskAi
				label={response.status >= 400 ? 'Why did this fail?' : 'Explain this response'}
				prompt={response.status >= 400
					? ASK.responseFailed(tab.id, response.status)
					: ASK.response(tab.id)}
				attach={{ request: true, response: true }}
			/>
		{/if}
		<HistoryMenu {tab} />
	</div>
</div>
