<script lang="ts">
	import type { Response } from '#lib/api/index.js';
	import { cn } from '#lib/utils.js';
	import { formatDuration } from '#lib/utils/format.js';
	import { statusColor } from '#lib/utils/http.js';

	interface Props {
		baseline: { label: string; detail: string; status: number; durationMs: number | null };
		current: Response;
	}

	let { baseline, current }: Props = $props();
</script>

<div class="grid grid-cols-2 gap-4 text-xs">
	<p>
		<span class="font-medium">{baseline.label}</span>
		<span class="text-muted-foreground">({baseline.detail})</span>
		<span class={cn('ml-2 font-mono', statusColor(baseline.status))}>{baseline.status}</span>
		{#if baseline.durationMs !== null}
			<span class="ml-2 text-muted-foreground">{formatDuration(baseline.durationMs)}</span>
		{/if}
	</p>
	<p>
		<span class="font-medium">Current response</span>
		<span class={cn('ml-2 font-mono', statusColor(current.status))}>{current.status}</span>
		<span class="ml-2 text-muted-foreground">{formatDuration(current.durationMs)}</span>
	</p>
</div>
