<script lang="ts">
	import PlusIcon from '@lucide/svelte/icons/plus';
	import type { Environment, WorkspaceWarning } from '#lib/api/index.js';
	import EnvBadge from '#lib/components/common/EnvBadge.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { cn } from '#lib/utils.js';

	interface Props {
		list: Environment[];
		/** Environment files left out of `list` because of their name. */
		skipped: WorkspaceWarning[];
		/** The environment being edited, if any. */
		current: string | undefined;
		showDefaults: boolean;
		onselect: (name: string) => void;
		ondefaults: () => void;
		oncreate: () => void;
	}

	let { list, skipped, current, showDefaults, onselect, ondefaults, oncreate }: Props = $props();
</script>

<div class="flex flex-col gap-1 border-r pr-3">
	<button
		type="button"
		class={cn(
			'rounded-md px-2 py-1.5 text-left text-sm hover:bg-muted',
			showDefaults && 'bg-muted font-medium'
		)}
		onclick={ondefaults}
	>
		Workspace defaults
	</button>
	<div class="my-1 border-t"></div>
	{#each list as env (env.name)}
		<button
			type="button"
			class={cn(
				'flex items-center justify-between rounded-md px-2 py-1.5 text-left text-sm hover:bg-muted',
				!showDefaults && current === env.name && 'bg-muted font-medium'
			)}
			onclick={() => onselect(env.name)}
		>
			<span class="truncate">{env.name}</span>
			{#if env.warning}
				<span class="text-xs text-destructive" title={env.warning}>can't read</span>
			{:else}
				<EnvBadge tag={env.tag} />
			{/if}
		</button>
	{/each}
	<Button size="sm" variant="ghost" class="justify-start" onclick={oncreate}>
		<PlusIcon /> New environment
	</Button>
	{#each skipped as file (file.path)}
		<p class="px-2 text-xs text-warning select-text">
			<code class="break-all">{file.path}</code>: {file.message}
		</p>
	{/each}
</div>
