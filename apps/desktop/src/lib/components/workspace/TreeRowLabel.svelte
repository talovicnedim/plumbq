<script lang="ts">
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import type { TreeNode } from '#lib/api/index.js';
	import { cn } from '#lib/utils.js';
	import { methodColor } from '#lib/utils/http.js';
	import { countRequests, shortMethod } from '#lib/utils/tree.js';

	interface Props {
		node: TreeNode;
		/** Folders only: whether its children are shown. */
		open: boolean;
		/** Requests only: the open tab has unsaved changes. */
		dirty: boolean;
	}

	let { node, open, dirty }: Props = $props();

	// The "..." button sits on top of the row end. While it shows (hover, keyboard focus, menu
	// open) the end gives way and a spacer keeps the name clear of the button. The 11px texts are
	// nudged down (translate-y) to share the name's baseline.
	const rowEnd =
		'flex items-center gap-2 group-hover:hidden group-has-focus-visible:hidden group-has-data-[state=open]:hidden';
</script>

{#if node.kind === 'folder'}
	<ChevronRightIcon
		class={cn('size-4 shrink-0 text-muted-foreground transition-transform', open && 'rotate-90')}
	/>
	{#if open}
		<FolderOpenIcon class="size-4 shrink-0 text-muted-foreground" />
	{:else}
		<FolderIcon class="size-4 shrink-0 text-muted-foreground" />
	{/if}
	<span class="min-w-0 flex-1 truncate font-medium text-sidebar-foreground">{node.name}</span>
	<span
		class={cn(rowEnd, 'shrink-0 translate-y-px text-[11px] text-muted-foreground tabular-nums')}
	>
		{countRequests(node)}
	</span>
{:else}
	<span class="size-4 shrink-0"></span>
	<span
		class={cn(
			'w-12 shrink-0 translate-y-[2px] truncate font-mono text-[11px] font-semibold tracking-tight',
			methodColor(node.method)
		)}
	>
		{shortMethod(node.method)}
	</span>
	<span class="min-w-16 flex-1 truncate">{node.name}</span>
	{#if node.tags.length > 0 || dirty}
		<span class={cn(rowEnd, 'min-w-0')}>
			{#if node.tags.length > 0}
				<span
					class="flex min-w-0 translate-y-px items-center gap-1 text-[11px] text-muted-foreground @max-[240px]:hidden"
				>
					<span class="max-w-24 min-w-0 truncate">#{node.tags[0]}</span>
					{#if node.tags.length > 1}<span class="shrink-0">+{node.tags.length - 1}</span>{/if}
				</span>
			{/if}
			{#if dirty}
				<span class="size-1.5 shrink-0 rounded-full bg-primary" title="Unsaved changes"></span>
			{/if}
		</span>
	{/if}
{/if}
<span
	class="hidden w-5 shrink-0 group-hover:block group-has-focus-visible:block group-has-data-[state=open]:block"
></span>
