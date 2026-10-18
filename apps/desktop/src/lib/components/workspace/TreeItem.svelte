<script lang="ts">
	import type { TreeNode } from '#lib/api/index.js';
	import * as ContextMenu from '#lib/components/ui/context-menu/index.js';
	import { moveRequest } from '#lib/state/actions.js';
	import { tabs } from '#lib/state/tabs.svelte.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { cn } from '#lib/utils.js';
	import { rowIndent } from '#lib/utils/tree.js';
	import { nodeMenu } from './menu.js';
	import Self from './TreeItem.svelte';
	import TreeGuides from './TreeGuides.svelte';
	import TreeItemMenu from './TreeItemMenu.svelte';
	import TreeRowLabel from './TreeRowLabel.svelte';

	interface Props {
		node: TreeNode;
		/** Path of the folder containing this node. */
		parent: string;
		/** Index among the parent's children, for drop positions. */
		index: number;
		depth: number;
		/** While filtering, every folder is shown open. */
		forceOpen: boolean;
	}

	let { node, parent, index, depth, forceOpen }: Props = $props();

	let dropTarget = $state(false);

	const menu = $derived(nodeMenu(node));
	const open = $derived(
		node.kind === 'folder' && (forceOpen || !workspace.collapsed.has(node.path))
	);
	const active = $derived(node.kind === 'request' && tabs.activeId === node.id);
	const dirty = $derived(
		node.kind === 'request' && tabs.all.some((t) => t.id === node.id && tabs.isDirty(t))
	);

	function ondragstart(e: DragEvent) {
		if (node.kind !== 'request' || !e.dataTransfer) return;
		e.dataTransfer.setData('text/x-request-id', node.id);
		e.dataTransfer.effectAllowed = 'move';
	}

	function ondragover(e: DragEvent) {
		if (!e.dataTransfer?.types.includes('text/x-request-id')) return;
		e.preventDefault();
		e.stopPropagation();
		dropTarget = true;
	}

	// Dropping on a folder moves into it; dropping on a request moves before it.
	function ondrop(e: DragEvent) {
		dropTarget = false;
		const id = e.dataTransfer?.getData('text/x-request-id');
		if (!id || (node.kind === 'request' && id === node.id)) return;
		e.preventDefault();
		e.stopPropagation();
		if (node.kind === 'folder') void moveRequest(id, node.path, null);
		else void moveRequest(id, parent, index);
	}

	function activate() {
		if (node.kind === 'folder') workspace.toggle(node.path);
		else void tabs.open(node.id);
	}
</script>

<ContextMenu.Root>
	<ContextMenu.Trigger class="mb-px">
		<div
			role="treeitem"
			aria-selected={active}
			aria-expanded={node.kind === 'folder' ? open : undefined}
			tabindex="-1"
			draggable={node.kind === 'request' && !forceOpen}
			class={cn(
				'group relative flex h-8 items-center rounded-md text-[14px] leading-5 text-sidebar-foreground/85 hover:bg-sidebar-foreground/6 has-data-[state=open]:bg-sidebar-foreground/6 has-[[data-tree-row]:focus-visible]:ring-2 has-[[data-tree-row]:focus-visible]:ring-ring',
				active &&
					'bg-primary/12 font-medium text-sidebar-foreground before:absolute before:inset-y-1.5 before:left-0 before:w-0.5 before:rounded-full before:bg-primary hover:bg-primary/12 has-data-[state=open]:bg-primary/12',
				dropTarget && node.kind === 'folder' && 'bg-primary/10 ring-1 ring-primary/50',
				dropTarget &&
					node.kind === 'request' &&
					'after:absolute after:inset-x-2 after:-top-px after:h-0.5 after:rounded after:bg-primary'
			)}
			{ondragstart}
			{ondragover}
			ondragleave={() => (dropTarget = false)}
			{ondrop}
		>
			<TreeGuides {depth} />
			<button
				type="button"
				class="flex h-full min-w-0 flex-1 items-center gap-2 overflow-hidden rounded-md pr-2 text-left focus-visible:outline-none"
				style:padding-left={`${rowIndent(depth)}rem`}
				onclick={activate}
				data-tree-row
				data-kind={node.kind}
				data-depth={depth}
				data-open={node.kind === 'folder' ? String(open) : undefined}
				data-key={node.kind === 'folder' ? node.path : node.id}
				data-name={node.name}
				title={node.kind === 'request' ? `${node.name}\n${node.url}` : node.path}
			>
				<TreeRowLabel {node} {open} {dirty} />
			</button>
			<TreeItemMenu {menu} />
		</div>
	</ContextMenu.Trigger>
	<ContextMenu.Content class="w-48">
		{#each menu as entry (entry.label)}
			{#if entry.separated}<ContextMenu.Separator />{/if}
			<ContextMenu.Item
				variant={entry.destructive ? 'destructive' : 'default'}
				onclick={() => entry.run()}
			>
				{entry.label}
			</ContextMenu.Item>
		{/each}
	</ContextMenu.Content>
</ContextMenu.Root>

{#if node.kind === 'folder' && open}
	{#each node.children as child, i (child.kind === 'folder' ? child.path : child.id)}
		<Self node={child} parent={node.path} index={i} depth={depth + 1} {forceOpen} />
	{/each}
{/if}
