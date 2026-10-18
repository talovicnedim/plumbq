<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { ai, type AiTab } from '#lib/state/ai.svelte.js';
	import { assistant } from '#lib/state/assistant.svelte.js';

	const LINKS: { tab: AiTab; label: string }[] = [
		{ tab: 'chat', label: 'Chat model' },
		{ tab: 'apps', label: 'AI apps' },
		{ tab: 'permissions', label: 'Permissions' },
		{ tab: 'activity', label: 'Activity' }
	];

	$effect(() => {
		void assistant.load();
		void ai.load();
	});
</script>

<div class="grid gap-3 text-sm">
	<p class="text-muted-foreground">
		{#if ai.off}
			AI is off.
		{:else}
			Chat: {assistant.provider
				? `${assistant.provider.name} · ${assistant.provider.model || 'no model'}`
				: 'no model picked'}. AI apps: {ai.running ? 'server running' : 'server off'}.
		{/if}
	</p>
	<div class="flex flex-wrap gap-2">
		{#each LINKS as link (link.tab)}
			<Button size="sm" variant="outline" onclick={() => ai.open(link.tab)}>{link.label}</Button>
		{/each}
	</div>
</div>
