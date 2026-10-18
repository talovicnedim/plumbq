<script lang="ts">
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type WorkspaceWarning } from '#lib/api/index.js';
	import { workspace } from '#lib/state/workspace.svelte.js';

	let warnings = $state<WorkspaceWarning[]>([]);
	let open = $state(false);

	// Re-checked whenever the tree reloads (the file watcher, edits): what had to be skipped.
	$effect(() => {
		if (!workspace.tree.data) return;
		api.workspace.warnings().then(
			(found) => (warnings = found),
			(err) => toast.error(toApiError(err).message)
		);
	});
</script>

{#if warnings.length > 0}
	<div class="px-2 pb-2 text-[12px]">
		<button
			type="button"
			class="flex h-8 w-full items-center gap-2 rounded-lg px-2.5 text-warning hover:bg-sidebar-foreground/6"
			onclick={() => (open = !open)}
		>
			<TriangleAlertIcon class="size-4" />
			{warnings.length === 1 ? '1 item skipped' : `${warnings.length} items skipped`}
		</button>
		{#if open}
			<ul class="mt-1 grid gap-1.5 rounded-lg border bg-background p-2.5 select-text">
				{#each warnings as warning, i (i)}
					<li>
						<code class="break-all">{warning.path || '(workspace root)'}</code>
						<p class="text-muted-foreground">{warning.message}</p>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
{/if}
