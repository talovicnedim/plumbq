<script lang="ts">
	import { Command as CommandPrimitive } from 'bits-ui';
	import { untrack } from 'svelte';
	import SearchIcon from '@lucide/svelte/icons/search';
	import PaletteRow from '#lib/components/palette/PaletteRow.svelte';
	import type { PaletteEntry } from '#lib/components/palette/entries.js';
	import { paletteSections } from '#lib/components/palette/sections.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import { palette } from '#lib/state/palette.svelte.js';
	import { parseQuery } from '#lib/utils/palette.js';

	interface Props {
		onmanageEnvironments: () => void;
		onconnectAi: () => void;
	}

	let { onmanageEnvironments, onconnectAi }: Props = $props();

	let search = $state('');
	let selected = $state('');

	// Each opening starts with an empty search.
	$effect(() => {
		if (palette.open) untrack(() => (search = ''));
	});

	const parsed = $derived(parseQuery(search));
	const searching = $derived(parsed.mode === 'all' && parsed.query !== '');
	const visible = $derived(
		paletteSections(search, {
			manageEnvironments: onmanageEnvironments,
			connectAi: onconnectAi
		}).filter((s) => s.items.length > 0)
	);

	// The first result is always the one Enter picks.
	$effect(() => {
		if (!palette.open) return;
		const first = visible[0]?.items[0]?.id ?? '';
		untrack(() => (selected = first));
	});

	function choose(entry: PaletteEntry) {
		if (entry.prefix) {
			search = entry.prefix;
			return;
		}
		// Closes first so dialogs opened by the action get focus.
		palette.open = false;
		void entry.run?.();
	}
</script>

<Command.Dialog
	bind:open={palette.open}
	bind:value={selected}
	shouldFilter={false}
	title="Command palette"
	description="Search requests, commands, environments and workspaces"
	class="top-[18%] sm:max-w-2xl"
>
	<div class="flex h-12 items-center gap-2 border-b px-3.5">
		<SearchIcon class="size-4 shrink-0 text-muted-foreground" />
		<CommandPrimitive.Input
			bind:value={search}
			placeholder="Search requests and commands…"
			class="h-full min-w-0 flex-1 bg-transparent text-[14px] outline-none placeholder:text-muted-foreground"
		/>
	</div>

	<Command.List class="max-h-[min(60vh,460px)] scroll-py-2 overflow-y-auto p-1.5">
		{#if visible.length === 0}
			<p class="py-10 text-center text-sm text-muted-foreground">
				Nothing matches “{parsed.query || search}”.
			</p>
		{/if}
		{#each visible as section (section.title)}
			<Command.Group
				heading={section.title}
				class="**:[[data-command-group-heading]]:px-2 **:[[data-command-group-heading]]:pt-2 **:[[data-command-group-heading]]:pb-1 **:[[data-command-group-heading]]:text-[11px] **:[[data-command-group-heading]]:font-medium **:[[data-command-group-heading]]:text-muted-foreground"
			>
				{#each section.items as entry (entry.id)}
					<Command.Item
						value={entry.id}
						onSelect={() => choose(entry)}
						class="h-11 gap-3 rounded-lg! px-2"
					>
						<PaletteRow {entry} showKind={searching} />
					</Command.Item>
				{/each}
			</Command.Group>
		{/each}
	</Command.List>

	<div
		class="flex h-9 items-center gap-4 border-t bg-muted/30 px-3.5 text-[11px] text-muted-foreground"
	>
		<span><kbd class="font-sans">↑↓</kbd> move</span>
		<span><kbd class="font-sans">↵</kbd> open</span>
		<span><kbd class="font-sans">esc</kbd> close</span>
		<span class="ml-auto flex gap-3">
			<span><kbd class="font-sans">&gt;</kbd> commands</span>
			<span><kbd class="font-sans">#</kbd> tags</span>
			<span><kbd class="font-sans">@</kbd> environments</span>
		</span>
	</div>
</Command.Dialog>
