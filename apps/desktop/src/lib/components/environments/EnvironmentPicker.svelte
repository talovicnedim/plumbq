<script lang="ts">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import EnvBadge from '#lib/components/common/EnvBadge.svelte';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { cn } from '#lib/utils.js';
	import { ENVIRONMENT_BG } from '#lib/utils/environments.js';

	interface Props {
		onmanage: () => void;
	}

	let { onmanage }: Props = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class={cn(
			'flex items-center gap-2 rounded-md border px-2.5 py-1 text-sm hover:bg-muted',
			environments.current?.tag === 'production' && 'border-env-production/60'
		)}
	>
		{#if environments.current}
			<EnvBadge tag={environments.current.tag} />
			{environments.current.name}
		{:else}
			<span class="text-muted-foreground">No environment</span>
		{/if}
		<ChevronDownIcon class="size-3.5 text-muted-foreground" />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="w-56">
		<DropdownMenu.Item onclick={() => environments.select(null)}>No environment</DropdownMenu.Item>
		<QueryView query={environments.list}>
			{#snippet children(list)}
				{#each list as env (env.name)}
					<DropdownMenu.Item onclick={() => environments.select(env.name)} class="gap-2">
						<span class={cn('size-2 rounded-full', ENVIRONMENT_BG[env.tag])}></span>
						{env.name}
					</DropdownMenu.Item>
				{/each}
			{/snippet}
		</QueryView>
		<DropdownMenu.Separator />
		{#if environments.skipped.length > 0}
			<DropdownMenu.Item onclick={onmanage} class="text-xs text-warning">
				{environments.skipped.length === 1
					? '1 environment file skipped'
					: `${environments.skipped.length} environment files skipped`}
			</DropdownMenu.Item>
		{/if}
		<DropdownMenu.Item onclick={onmanage}>Manage environments…</DropdownMenu.Item>
	</DropdownMenu.Content>
</DropdownMenu.Root>
