<script lang="ts">
	import SendIcon from '@lucide/svelte/icons/send-horizontal';
	import SquareIcon from '@lucide/svelte/icons/square';
	import { Button } from '#lib/components/ui/button/index.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
	import { tabs } from '#lib/state/tabs.svelte.js';
	import { cn } from '#lib/utils.js';

	let input = $state('');

	const chips = $derived([
		{ id: 'request' as const, label: 'This request', available: !!tabs.active },
		{ id: 'folder' as const, label: 'This folder', available: !!tabs.active },
		{ id: 'response' as const, label: 'Last response', available: !!tabs.active?.entry }
	]);

	function submit() {
		const text = input;
		input = '';
		void assistant.ask(text);
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
			e.preventDefault();
			submit();
		}
	}
</script>

<div class="shrink-0 border-t p-2">
	<div class="mb-1.5 flex flex-wrap gap-1">
		{#each chips.filter((c) => c.available) as chip (chip.id)}
			<button
				type="button"
				class={cn(
					'rounded-full border px-2 py-0.5 text-[11px]',
					assistant.chips[chip.id]
						? 'border-primary/50 bg-primary/10 text-primary'
						: 'text-muted-foreground'
				)}
				aria-pressed={assistant.chips[chip.id]}
				onclick={() => (assistant.chips[chip.id] = !assistant.chips[chip.id])}
			>
				{chip.label}
			</button>
		{/each}
	</div>
	<form
		class="flex items-end gap-1.5 rounded-lg border bg-background p-1.5 focus-within:ring-2 focus-within:ring-ring/40"
		onsubmit={(e) => {
			e.preventDefault();
			submit();
		}}
	>
		<textarea
			class="max-h-40 min-h-9 flex-1 resize-none bg-transparent px-1.5 py-1 text-sm outline-none"
			rows="2"
			placeholder="Ask about this workspace... (Enter to send, Shift+Enter for a new line)"
			bind:value={input}
			{onkeydown}></textarea>
		{#if assistant.sending}
			<Button type="button" size="sm" variant="outline" onclick={() => assistant.stop()}>
				<SquareIcon /> Stop
			</Button>
		{:else}
			<Button type="submit" size="sm" disabled={!input.trim()} aria-label="Send">
				<SendIcon />
			</Button>
		{/if}
	</form>
</div>
