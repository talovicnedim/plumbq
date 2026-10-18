<script lang="ts">
	import CheckIcon from '@lucide/svelte/icons/check';
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type McpAppStatus } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';

	interface Props {
		status: McpAppStatus;
		/** Called after a connect or disconnect, to reload the list. */
		onchange: () => void;
	}

	let { status, onchange }: Props = $props();

	let busy = $state(false);

	async function toggle() {
		busy = true;
		const remove = status.connected && !status.outdated;
		try {
			if (remove) await api.mcp.apps.disconnect(status.app);
			else await api.mcp.apps.connect(status.app);
			toast.success(remove ? `Removed from ${status.label}` : `Added to ${status.label}`, {
				description: remove ? undefined : `Restart ${status.label} to start using it there.`
			});
			onchange();
		} catch (err) {
			toast.error(toApiError(err).message);
		} finally {
			busy = false;
		}
	}
</script>

<div class="flex items-center gap-3 rounded-lg border px-3 py-2.5">
	<div class="min-w-0 flex-1">
		<p class="font-medium">{status.label}</p>
		<p class="truncate text-xs text-muted-foreground" title={status.configPath}>
			{#if status.error}
				<span class="text-destructive">{status.error}</span>
			{:else if status.outdated}
				Set up with old settings
			{:else if status.connected}
				<span class="inline-flex items-center gap-1 text-success"
					><CheckIcon class="size-3" /> Connected</span
				>
			{:else if status.installed}
				Not connected
			{:else}
				Not found on this computer
			{/if}
		</p>
	</div>
	<Button
		size="sm"
		variant={status.connected && !status.outdated ? 'ghost' : 'default'}
		disabled={busy || (!status.installed && !status.connected)}
		onclick={toggle}
	>
		{status.outdated ? 'Update' : status.connected ? 'Disconnect' : 'Connect'}
	</Button>
</div>
