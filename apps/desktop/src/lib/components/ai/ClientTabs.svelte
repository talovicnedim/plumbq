<script lang="ts">
	import type { McpClient } from '#lib/api/index.js';
	import { cn } from '#lib/utils.js';

	interface Props {
		clients: { value: McpClient; label: string }[];
		value: McpClient;
	}

	let { clients, value = $bindable() }: Props = $props();
</script>

<!-- Smaller than the dialog's own tabs and wrapping, so six clients never scroll sideways. -->
<div class="flex flex-wrap gap-1 rounded-lg bg-muted p-1" role="tablist" aria-label="AI app">
	{#each clients as client (client.value)}
		<button
			type="button"
			role="tab"
			aria-selected={value === client.value}
			class={cn(
				'flex-[1_1_auto] rounded-md px-3 py-1 text-[13px] font-medium whitespace-nowrap text-muted-foreground transition-colors hover:text-foreground',
				value === client.value && 'bg-background text-foreground shadow-sm'
			)}
			onclick={() => (value = client.value)}
		>
			{client.label}
		</button>
	{/each}
</div>
