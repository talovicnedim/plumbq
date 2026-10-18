<script lang="ts">
	import { labelFor } from '#lib/utils/shortcuts.js';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import {
		importCurl,
		moveRequest,
		newFolder,
		newRequest,
		removeFolder,
		removeRequest,
		renameFolder,
		renameRequest
	} from '#lib/state/actions.js';
	import { treeKeyAction } from '#lib/utils/tree-keys.js';
	import { importer } from '#lib/state/importer.svelte.js';
	import { workspaceSettings } from '#lib/state/workspace-settings.svelte.js';
	import { palette } from '#lib/state/palette.svelte.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { isMac } from '#lib/utils/platform.js';
	import { filterTree } from '#lib/utils/search.js';
	import TreeItem from './TreeItem.svelte';
	import TreeWarnings from './TreeWarnings.svelte';
	import WorkspaceSwitcher from './WorkspaceSwitcher.svelte';

	interface Props {
		/** Opens the workspace switcher, e.g. from ⌘P. */
		switcherOpen?: boolean;
	}

	let { switcherOpen = $bindable(false) }: Props = $props();

	let filter = $state('');

	/** Keyboard navigation over the visible rows (see `treeKeyAction`). */
	function onTreeKeydown(e: KeyboardEvent) {
		const container = e.currentTarget;
		if (!(container instanceof HTMLElement)) return;
		const buttons = [...container.querySelectorAll<HTMLButtonElement>('[data-tree-row]')];
		const index = buttons.findIndex((b) => b === document.activeElement);
		if (index === -1) return;
		const rows = buttons.map((b) => ({
			kind: b.dataset.kind === 'folder' ? ('folder' as const) : ('request' as const),
			depth: Number(b.dataset.depth),
			open: b.dataset.open === 'true'
		}));
		const action = treeKeyAction(e.key, rows, index);
		if (!action) return;
		e.preventDefault();
		const row = buttons[index];
		const key = row.dataset.key ?? '';
		const name = row.dataset.name ?? '';
		const folder = row.dataset.kind === 'folder';
		if (action.type === 'focus') buttons[action.index]?.focus();
		else if (action.type === 'toggle') workspace.toggle(key);
		else if (action.type === 'rename')
			void (folder ? renameFolder(key, name) : renameRequest(key, name));
		else void (folder ? removeFolder(key, name) : removeRequest(key, name));
	}

	// Dropping on empty space below the tree moves to the workspace root.
	function ondrop(e: DragEvent) {
		const id = e.dataTransfer?.getData('text/x-request-id');
		if (!id) return;
		e.preventDefault();
		void moveRequest(id, '', null);
	}
</script>

<aside class="flex h-full min-h-0 flex-col bg-sidebar">
	{#if isMac}
		<!-- Room for the traffic lights of the overlay title bar; also drags the window. -->
		<div data-tauri-drag-region class="h-9 shrink-0"></div>
	{:else}
		<div class="h-2 shrink-0"></div>
	{/if}
	<div class="flex items-center gap-1 px-2 pb-2">
		<WorkspaceSwitcher bind:open={switcherOpen} />
		<DropdownMenu.Root>
			<DropdownMenu.Trigger
				class="flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-sidebar-foreground/6 hover:text-foreground data-[state=open]:bg-sidebar-foreground/6 data-[state=open]:text-foreground"
				aria-label="Add"
				title="New request, folder or import"
			>
				<PlusIcon class="size-4" />
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end" class="w-56">
				<DropdownMenu.Item onclick={() => newRequest('')}>
					New request <DropdownMenu.Shortcut>{labelFor('newRequest')}</DropdownMenu.Shortcut>
				</DropdownMenu.Item>
				<DropdownMenu.Item onclick={() => newFolder('')}>New folder</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item onclick={() => importCurl('')}>Import cURL…</DropdownMenu.Item>
				<DropdownMenu.Item onclick={() => importer.show('', 'file')}>
					Import Postman, OpenAPI, Insomnia…
				</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item onclick={() => (workspaceSettings.open = true)}>
					Workspace settings…
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	</div>

	<div class="px-2 pb-2">
		<label
			class="flex h-9 items-center gap-2 rounded-lg border bg-background/60 px-2.5 text-[13px] focus-within:ring-1 focus-within:ring-ring"
		>
			<SearchIcon class="size-4 shrink-0 text-muted-foreground" />
			<input
				class="min-w-0 flex-1 bg-transparent text-ellipsis outline-none placeholder:text-muted-foreground"
				placeholder="Filter requests or tag:name"
				bind:value={filter}
				spellcheck="false"
				onkeydown={(e) => e.key === 'Escape' && (filter = '')}
			/>
			{#if filter}
				<button
					type="button"
					class="text-muted-foreground hover:text-foreground"
					aria-label="Clear filter"
					onclick={() => (filter = '')}
				>
					<XIcon class="size-4" />
				</button>
			{:else}
				<button
					type="button"
					class="rounded border bg-muted px-1.5 text-[10px] text-muted-foreground hover:text-foreground"
					title="Search everything"
					onclick={() => (palette.open = true)}
				>
					{labelFor('palette')}
				</button>
			{/if}
		</label>
	</div>

	<TreeWarnings />

	<div
		role="tree"
		aria-label="Requests"
		tabindex="-1"
		onkeydown={onTreeKeydown}
		class="@container min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-2 pt-1 pb-4"
		ondragover={(e) => e.preventDefault()}
		{ondrop}
	>
		<QueryView
			query={workspace.tree}
			empty="No requests yet. Use + to create one or import a collection."
			isEmpty={(tree) => tree.length === 0}
		>
			{#snippet children(tree)}
				{@const shown = filterTree(tree, filter)}
				{#each shown as node, i (node.kind === 'folder' ? node.path : node.id)}
					<TreeItem {node} parent="" index={i} depth={0} forceOpen={!!filter} />
				{:else}
					<p class="px-2 py-6 text-center text-[13px] text-muted-foreground">
						No requests match "{filter}".
					</p>
				{/each}
			{/snippet}
		</QueryView>
	</div>
</aside>
