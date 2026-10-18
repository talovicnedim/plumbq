<script lang="ts">
	import CircleDotIcon from '@lucide/svelte/icons/circle-dot';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import KeyboardIcon from '@lucide/svelte/icons/keyboard';
	import EnvBadge from '#lib/components/common/EnvBadge.svelte';
	import { reveal } from '#lib/state/actions.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { overlays } from '#lib/state/overlays.svelte.js';
	import { recentRequests } from '#lib/state/recent-requests.svelte.js';
	import { statusItems } from '#lib/state/status-items.svelte.js';
	import { tabs } from '#lib/state/tabs.svelte.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { cn } from '#lib/utils.js';
	import { formatTime } from '#lib/utils/format.js';
	import { statusColor } from '#lib/utils/http.js';
	import { flatten } from '#lib/utils/search.js';
	import { labelFor } from '#lib/utils/shortcuts.js';

	const dirty = $derived(tabs.all.filter((t) => tabs.isDirty(t)).length);
	const lastSent = $derived(recentRequests.list.find((r) => r.sentAt !== undefined));
	const lastName = $derived(
		lastSent
			? (flatten(workspace.tree.data ?? []).find((r) => r.id === lastSent.id)?.name ?? lastSent.id)
			: ''
	);
</script>

<footer
	class="flex h-6 shrink-0 items-center gap-3 border-t bg-sidebar px-3 text-[11px] text-muted-foreground"
>
	<button
		type="button"
		class="flex min-w-0 items-center gap-1 hover:text-foreground"
		title="Reveal the workspace folder"
		onclick={() => reveal('')}
	>
		<FolderIcon class="size-3 shrink-0" />
		<span class="truncate">{workspace.root}</span>
	</button>

	{#if environments.current}
		<span class="flex items-center gap-1">
			<EnvBadge tag={environments.current.tag} class="h-4 px-1 text-[9px]" />
			{environments.current.name}
		</span>
	{:else}
		<span>No environment</span>
	{/if}

	{#if dirty > 0}
		<span class="flex items-center gap-1 text-foreground" title="Tabs with unsaved changes">
			<CircleDotIcon class="size-3 text-primary" />
			{dirty} unsaved
		</span>
	{/if}

	{#if lastSent?.sentAt}
		<button
			type="button"
			class="flex min-w-0 items-center gap-1 hover:text-foreground"
			title="Open the last sent request"
			onclick={() => tabs.open(lastSent.id)}
		>
			Last sent:
			<span class={cn('font-mono font-semibold', statusColor(lastSent.status ?? 0))}>
				{lastSent.status}
			</span>
			<span class="truncate">{lastName}</span>
			<span>{formatTime(new Date(lastSent.sentAt).toISOString())}</span>
		</button>
	{/if}

	<div class="ml-auto flex items-center gap-3">
		{#each statusItems.items as item (item.id)}
			<item.component />
		{/each}
		<button
			type="button"
			class="flex items-center gap-1 hover:text-foreground"
			title={`Keyboard shortcuts (${labelFor('shortcuts')})`}
			aria-label="Keyboard shortcuts"
			onclick={() => (overlays.shortcuts = true)}
		>
			<KeyboardIcon class="size-3" />
			{labelFor('shortcuts')}
		</button>
	</div>
</footer>
