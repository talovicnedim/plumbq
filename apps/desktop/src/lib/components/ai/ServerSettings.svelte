<script lang="ts">
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import type { McpClientSetup } from '#lib/api/index.js';
	import CopyField from '#lib/components/common/CopyField.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import { confirm } from '#lib/state/confirm.svelte.js';
	import DevBuildHint from './DevBuildHint.svelte';

	interface Props {
		/** Setup for the headless stdio server, once the server runs. */
		stdio: McpClientSetup | undefined;
	}

	let { stdio }: Props = $props();

	// Follows the saved port until the user types another one.
	let port = $derived(String(ai.status?.settings.port ?? ''));
	const portNumber = $derived(Number.parseInt(port, 10));

	async function regenerate() {
		const ok = await confirm({
			title: 'Regenerate the server address?',
			description:
				'AI apps set up with the current address stop working until you add the new one.',
			confirmLabel: 'Regenerate'
		});
		if (ok) await ai.regenerateToken();
	}
</script>

<details class="group rounded-lg border">
	<summary class="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-sm font-medium">
		<ChevronRightIcon class="size-4 transition-transform group-open:rotate-90" />
		Server settings
	</summary>
	<div class="grid gap-4 border-t p-3">
		<div class="flex items-end gap-3">
			<label class="grid gap-1.5">
				Port
				<Input bind:value={port} inputmode="numeric" class="h-8 w-28" />
			</label>
			<Button
				variant="outline"
				size="sm"
				disabled={Number.isNaN(portNumber) || portNumber === ai.status?.settings.port}
				onclick={() => ai.update({ port: portNumber })}>Change port</Button
			>
		</div>
		<p class="text-muted-foreground">The address includes a secret, so treat it like a password.</p>
		<div class="flex gap-2">
			<Button variant="outline" size="sm" onclick={regenerate}>
				<RefreshCwIcon /> Regenerate address
			</Button>
			<Button variant="ghost" size="sm" onclick={() => ai.update({ enabled: false })}>
				Turn off server
			</Button>
		</div>
		{#if stdio}
			<div class="grid gap-2 border-t pt-3">
				<p class="font-medium">Run without the app (headless)</p>
				<p class="text-xs text-muted-foreground">{stdio.hint}</p>
				<CopyField value={stdio.snippet} multiline />
				{#if stdio.note}<p class="text-xs text-muted-foreground">{stdio.note}</p>{/if}
				<DevBuildHint />
			</div>
		{/if}
	</div>
</details>
