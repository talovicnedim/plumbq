<script lang="ts">
	import PlusIcon from '@lucide/svelte/icons/plus';
	import SettingsIcon from '@lucide/svelte/icons/settings-2';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import XIcon from '@lucide/svelte/icons/x';
	import { Button } from '#lib/components/ui/button/index.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
	import { providerMeta } from '#lib/utils/providers.js';
	import { labelFor } from '#lib/utils/shortcuts.js';
	import ChatAnswer from './ChatAnswer.svelte';
	import ChatComposer from './ChatComposer.svelte';
	import ChatHistory from './ChatHistory.svelte';
	import ToolCard from './ToolCard.svelte';

	let scroller = $state<HTMLDivElement | null>(null);

	$effect(() => {
		void assistant.load();
	});

	// Follow the stream to the bottom.
	$effect(() => {
		void assistant.items.length;
		void JSON.stringify(assistant.items.at(-1));
		scroller?.scrollTo({ top: scroller.scrollHeight });
	});
</script>

<aside class="flex h-full min-h-0 flex-col border-l bg-background" aria-label="AI chat">
	<div class="flex h-11 shrink-0 items-center gap-1.5 border-b px-3">
		<SparklesIcon class="size-4 shrink-0 text-primary" />
		{#if assistant.provider}
			<span
				class="truncate text-xs font-medium"
				title={assistant.providerStatus?.local ? 'Runs on this machine' : 'Cloud, with your key'}
			>
				{providerMeta(assistant.provider.kind)?.label ?? assistant.provider.name} · {assistant
					.provider.model || 'no model'}
			</span>
		{:else}
			<span class="text-sm font-semibold">Chat</span>
		{/if}
		<div class="ml-auto flex items-center">
			<ChatHistory />
			<button
				type="button"
				class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
				aria-label="New chat"
				title="New chat"
				onclick={() => assistant.newChat()}
			>
				<PlusIcon class="size-4" />
			</button>
			<button
				type="button"
				class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
				aria-label="AI settings"
				title="AI settings"
				onclick={() => ai.open('chat')}
			>
				<SettingsIcon class="size-4" />
			</button>
			<button
				type="button"
				class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
				aria-label="Close chat"
				title={`Close (${labelFor('assistant')})`}
				onclick={() => assistant.toggle()}
			>
				<XIcon class="size-4" />
			</button>
		</div>
	</div>

	{#if ai.off}
		<div class="grid gap-3 p-4 text-sm">
			<p class="font-medium">AI is off</p>
			<p class="text-muted-foreground">Turn it on to chat. Your settings and keys are kept.</p>
			<Button size="sm" onclick={() => ai.turnOn()}>Turn AI on</Button>
		</div>
	{:else if !assistant.ready}
		<div class="grid gap-3 p-4 text-sm">
			{#if assistant.pending}
				<p class="rounded-md bg-muted px-3 py-2 text-xs">
					<span class="text-muted-foreground">Waiting to ask:</span>
					{assistant.pending}
				</p>
			{/if}
			<p class="font-medium">Pick a model to chat</p>
			<p class="text-muted-foreground">
				A local model in Ollama or LM Studio, or your own Anthropic, OpenAI or Gemini key. The chat
				uses the same tools and approvals as AI apps.
			</p>
			<Button size="sm" onclick={() => ai.open('chat')}>Pick a model</Button>
			<button
				type="button"
				class="text-left text-xs text-muted-foreground hover:text-foreground hover:underline"
				onclick={() => ai.open('apps')}
			>
				Or use Claude Code, Cursor, VS Code, Codex or ChatGPT
			</button>
		</div>
	{:else}
		<div bind:this={scroller} class="min-h-0 flex-1 overflow-y-auto px-3 py-3">
			{#if assistant.items.length === 0}
				<div class="grid gap-2 text-sm text-muted-foreground">
					<p>Ask about this workspace, or start with:</p>
					<div class="flex flex-wrap gap-1.5">
						{#each assistant.quickActions as action (action.name)}
							<button
								type="button"
								class="rounded-full border px-2.5 py-1 text-xs hover:bg-muted"
								title={action.description}
								onclick={() => assistant.runQuickAction(action)}
							>
								{action.name.replace(/_/g, ' ')}
							</button>
						{/each}
					</div>
				</div>
			{/if}
			<div class="grid gap-3">
				{#each assistant.items as item, i (i)}
					{#if item.kind === 'user'}
						<div class="ml-8 rounded-lg bg-muted px-3 py-2 text-sm whitespace-pre-wrap select-text">
							{item.text}
						</div>
					{:else if item.kind === 'assistant'}
						<ChatAnswer text={item.text} />
					{:else}
						<ToolCard {item} />
					{/if}
				{/each}
				{#if assistant.error}
					<p
						class="rounded-md border border-destructive/40 bg-destructive/5 px-3 py-2 text-xs text-destructive"
					>
						{assistant.error}
					</p>
				{/if}
			</div>
		</div>
		<ChatComposer />
	{/if}
</aside>
