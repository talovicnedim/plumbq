<script lang="ts">
	import { Command as CommandPrimitive } from 'bits-ui';
	import { untrack } from 'svelte';
	import SearchIcon from '@lucide/svelte/icons/search';
	import PaletteRow from '#lib/components/palette/PaletteRow.svelte';
	import type { PaletteEntry } from '#lib/components/palette/entries.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import { overlays } from '#lib/state/overlays.svelte.js';
	import { recentRequests } from '#lib/state/recent-requests.svelte.js';
	import { tabs } from '#lib/state/tabs.svelte.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { formatTime } from '#lib/utils/format.js';
	import { rank } from '#lib/utils/palette.js';
	import { flatten } from '#lib/utils/search.js';

	let search = $state('');
	let selected = $state('');

	$effect(() => {
		if (overlays.recent) untrack(() => (search = ''));
	});

	/** Recent requests that still exist, with their current names. */
	const entries = $derived.by(() => {
		const byId = new Map(flatten(workspace.tree.data ?? []).map((r) => [r.id, r]));
		return recentRequests.list.flatMap((recent): PaletteEntry[] => {
			const request = byId.get(recent.id);
			if (!request) return [];
			return [
				{
					id: recent.id,
					kind: 'Request',
					method: request.method,
					title: request.name,
					subtitle: request.folder,
					keywords: `${request.url} ${request.id}`,
					status: recent.status,
					meta: formatTime(new Date(recent.at).toISOString()),
					run: () => tabs.open(recent.id)
				}
			];
		});
	});
	const shown = $derived(rank(entries, search));

	$effect(() => {
		if (!overlays.recent) return;
		const first = shown[0]?.id ?? '';
		untrack(() => (selected = first));
	});

	function choose(entry: PaletteEntry) {
		overlays.recent = false;
		void entry.run?.();
	}
</script>

<Command.Dialog
	bind:open={overlays.recent}
	bind:value={selected}
	shouldFilter={false}
	title="Recent requests"
	description="Requests you opened or sent lately"
	class="top-[18%] sm:max-w-xl"
>
	<div class="flex h-12 items-center gap-2 border-b px-3.5">
		<SearchIcon class="size-4 shrink-0 text-muted-foreground" />
		<CommandPrimitive.Input
			bind:value={search}
			placeholder="Recent requests…"
			class="h-full min-w-0 flex-1 bg-transparent text-[14px] outline-none placeholder:text-muted-foreground"
		/>
	</div>
	<Command.List class="max-h-[min(60vh,460px)] scroll-py-2 overflow-y-auto p-1.5">
		{#if shown.length === 0}
			<p class="py-10 text-center text-sm text-muted-foreground">
				{entries.length === 0
					? 'Nothing opened yet in this workspace.'
					: `Nothing matches “${search}”.`}
			</p>
		{/if}
		{#each shown as entry (entry.id)}
			<Command.Item
				value={entry.id}
				onSelect={() => choose(entry)}
				class="h-11 gap-3 rounded-lg! px-2"
			>
				<PaletteRow {entry} showKind={false} />
			</Command.Item>
		{/each}
	</Command.List>
</Command.Dialog>
