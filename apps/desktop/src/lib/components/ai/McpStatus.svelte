<script lang="ts">
	import BotIcon from '@lucide/svelte/icons/bot';
	import { ai } from '#lib/state/ai.svelte.js';
	import { cn } from '#lib/utils.js';
</script>

<button
	type="button"
	class="flex items-center gap-1.5 rounded px-1.5 py-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
	title={ai.running ? `MCP server running at ${ai.status?.url}` : 'MCP server off'}
	onclick={() => ai.open(ai.pending.length > 0 ? 'activity' : 'apps')}
>
	<BotIcon class="size-3.5" />
	<span class={cn('size-1.5 rounded-full', ai.running ? 'bg-success' : 'bg-muted-foreground/40')}
	></span>
	MCP
	{#if ai.pending.length > 0}
		<span class="rounded-full bg-warning px-1.5 text-[10px] font-semibold text-white">
			{ai.pending.length}
		</span>
	{/if}
</button>
