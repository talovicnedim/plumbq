<script lang="ts">
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import Segmented from '#lib/components/common/Segmented.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { ai, type AiTab } from '#lib/state/ai.svelte.js';
	import { confirm } from '#lib/state/confirm.svelte.js';
	import ActivityTab from './ActivityTab.svelte';
	import AppsTab from './AppsTab.svelte';
	import ChatTab from './ChatTab.svelte';
	import PermissionsTab from './PermissionsTab.svelte';

	const TABS: { value: AiTab; label: string }[] = [
		{ value: 'chat', label: 'Chat' },
		{ value: 'apps', label: 'AI apps' },
		{ value: 'permissions', label: 'Permissions' },
		{ value: 'activity', label: 'Activity' }
	];

	async function turnOff() {
		const ok = await confirm({
			title: 'Turn AI off?',
			description:
				'The chat, the MCP server for AI apps and the "Explain" buttons go away until you turn AI back on. Your settings and keys are kept, and the server comes back on with AI.',
			confirmLabel: 'Turn off'
		});
		if (ok) await ai.turnOff();
	}
</script>

<Dialog.Root bind:open={ai.dialogOpen}>
	<Dialog.Content class="flex h-[min(88vh,760px)] flex-col sm:max-w-[min(56rem,calc(100%-2rem))]">
		<Dialog.Header>
			<div class="flex items-baseline justify-between pr-8">
				<Dialog.Title>AI</Dialog.Title>
				{#if !ai.off}
					<button
						type="button"
						class="text-xs text-muted-foreground hover:underline"
						onclick={turnOff}
					>
						Turn AI off
					</button>
				{/if}
			</div>
			<Dialog.Description>
				Chat here with your own model, or let your AI app work in Plumbq. Both use the same tools,
				ask you before changes, and show up in Activity.
			</Dialog.Description>
		</Dialog.Header>

		{#if ai.off}
			<div class="flex items-center justify-between gap-6 rounded-xl border p-5 text-sm">
				<div class="flex items-start gap-3">
					<SparklesIcon class="mt-0.5 size-4 text-muted-foreground" />
					<div>
						<p class="font-medium">AI is off</p>
						<p class="text-muted-foreground">
							No chat, no MCP server and no "Explain" buttons. Your settings and keys are kept.
						</p>
					</div>
				</div>
				<Button onclick={() => ai.turnOn()}>Turn AI on</Button>
			</div>
		{:else}
			<Segmented options={TABS} bind:value={ai.tab} />
			<div class="min-h-0 flex-1 overflow-x-hidden overflow-y-auto pr-1">
				{#if ai.tab === 'chat'}
					<ChatTab />
				{:else if ai.tab === 'apps'}
					<AppsTab />
				{:else if ai.tab === 'permissions'}
					<PermissionsTab />
				{:else}
					<ActivityTab />
				{/if}
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
