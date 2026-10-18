<script lang="ts">
	import { labelFor } from '#lib/utils/shortcuts.js';
	import XIcon from '@lucide/svelte/icons/x';
	import * as ContextMenu from '#lib/components/ui/context-menu/index.js';
	import { tabs } from '#lib/state/tabs.svelte.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { cn } from '#lib/utils.js';
	import { methodColor } from '#lib/utils/http.js';
	import TabListMenu from './TabListMenu.svelte';

	const DRAG_TYPE = 'text/x-tab-id';

	/** Index the dragged tab would land at, for the drop marker. */
	let dropIndex = $state<number | null>(null);
	let strip = $state<HTMLElement | null>(null);
	/** More tabs than fit: show the menu that lists them all. */
	let overflowing = $state(false);

	// Keeps the active tab in view, also when it is opened from the sidebar or the palette.
	$effect(() => {
		const id = tabs.activeId;
		if (id) scrollToTab(id);
	});

	/** Picked from the list of all tabs: it may already be active, so scroll to it directly. */
	function show(id: string) {
		tabs.activeId = id;
		scrollToTab(id);
	}

	function scrollToTab(id: string) {
		const tab = [...(strip?.querySelectorAll<HTMLElement>('[data-tab-id]') ?? [])].find(
			(el) => el.dataset.tabId === id
		);
		tab?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
	}

	$effect(() => {
		if (!strip) return;
		const el = strip;
		void tabs.all.length;
		const measure = () => (overflowing = el.scrollWidth > el.clientWidth + 1);
		const observer = new ResizeObserver(measure);
		observer.observe(el);
		measure();
		return () => observer.disconnect();
	});

	// A mouse wheel scrolls the tabs sideways. Svelte's `onwheel` is passive, so it couldn't stop
	// the page from scrolling instead.
	$effect(() => {
		if (!strip) return;
		const el = strip;
		const onwheel = (e: WheelEvent) => {
			if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
			el.scrollLeft += e.deltaY;
			e.preventDefault();
		};
		el.addEventListener('wheel', onwheel, { passive: false });
		return () => el.removeEventListener('wheel', onwheel);
	});

	/** Opens every folder above the request so it's visible (and highlighted) in the sidebar. */
	function reveal(id: string) {
		const parts = id.split('#')[0].split('/').slice(0, -1);
		const next = new Set(workspace.collapsed);
		parts.forEach((_, i) => next.delete(parts.slice(0, i + 1).join('/')));
		workspace.collapsed = next;
		tabs.activeId = id;
	}

	function ondragover(e: DragEvent, index: number) {
		if (!e.dataTransfer?.types.includes(DRAG_TYPE)) return;
		if (!(e.currentTarget instanceof HTMLElement)) return;
		e.preventDefault();
		const rect = e.currentTarget.getBoundingClientRect();
		dropIndex = e.clientX < rect.left + rect.width / 2 ? index : index + 1;
	}

	function ondrop(e: DragEvent) {
		const id = e.dataTransfer?.getData(DRAG_TYPE);
		if (id && dropIndex !== null) {
			e.preventDefault();
			const from = tabs.all.findIndex((t) => t.id === id);
			// Removing the tab first shifts later positions left by one.
			tabs.move(id, from < dropIndex ? dropIndex - 1 : dropIndex);
		}
		dropIndex = null;
	}
</script>

<nav
	class="flex h-9 shrink-0 items-stretch border-b bg-muted/30 text-[13px]"
	ondragleave={(e) => {
		const inside = e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget);
		if (!inside) dropIndex = null;
	}}
>
	<div
		bind:this={strip}
		class="flex min-w-0 flex-1 [scrollbar-width:none] items-stretch overflow-x-auto overflow-y-hidden [&::-webkit-scrollbar]:hidden"
	>
		{#each tabs.all as tab, index (tab.id)}
			{@const active = tab.id === tabs.activeId}
			<ContextMenu.Root>
				<ContextMenu.Trigger>
					<div
						role="tab"
						tabindex="-1"
						aria-selected={active}
						draggable="true"
						data-tab-id={tab.id}
						class={cn(
							'group relative flex h-full max-w-56 min-w-28 flex-[0_1_14rem] items-center border-r',
							active
								? 'bg-background text-foreground after:absolute after:inset-x-0 after:top-0 after:h-0.5 after:bg-primary'
								: 'text-muted-foreground hover:bg-muted',
							dropIndex === index && 'shadow-[inset_2px_0_0_var(--primary)]',
							dropIndex === index + 1 &&
								index === tabs.all.length - 1 &&
								'shadow-[inset_-2px_0_0_var(--primary)]'
						)}
						ondragstart={(e) => e.dataTransfer?.setData(DRAG_TYPE, tab.id)}
						ondragover={(e) => ondragover(e, index)}
						{ondrop}
						ondragend={() => (dropIndex = null)}
					>
						<button
							type="button"
							class="flex min-w-0 flex-1 items-center gap-1.5 py-1 pr-1 pl-3 text-left"
							title={tab.draft.name || tab.id}
							onclick={() => (tabs.activeId = tab.id)}
							onauxclick={(e) => e.button === 1 && tabs.close(tab.id)}
						>
							<span
								class={cn('font-mono text-[10px] font-semibold', methodColor(tab.draft.method))}
							>
								{tab.draft.method}
							</span>
							<span class="truncate">{tab.draft.name || tab.id}</span>
						</button>
						<button
							type="button"
							class="mr-1.5 rounded p-0.5 hover:bg-muted"
							aria-label="Close tab"
							onclick={() => tabs.close(tab.id)}
						>
							{#if tabs.isDirty(tab)}
								<span class="block size-3.5 group-hover:hidden">
									<span class="m-auto mt-1 block size-1.5 rounded-full bg-foreground/60"></span>
								</span>
								<XIcon class="hidden size-3.5 group-hover:block" />
							{:else}
								<XIcon class={cn('size-3.5', !active && 'opacity-0 group-hover:opacity-100')} />
							{/if}
						</button>
					</div>
				</ContextMenu.Trigger>
				<ContextMenu.Content class="w-52">
					<ContextMenu.Item onclick={() => tabs.close(tab.id)}>
						Close <ContextMenu.Shortcut>{labelFor('closeTab')}</ContextMenu.Shortcut>
					</ContextMenu.Item>
					<ContextMenu.Item disabled={tabs.all.length < 2} onclick={() => tabs.closeOthers(tab.id)}>
						Close others
					</ContextMenu.Item>
					<ContextMenu.Item
						disabled={index === tabs.all.length - 1}
						onclick={() => tabs.closeToRight(tab.id)}
					>
						Close to the right
					</ContextMenu.Item>
					<ContextMenu.Item onclick={() => tabs.closeEverything()}>
						Close all <ContextMenu.Shortcut>{labelFor('closeAllTabs')}</ContextMenu.Shortcut>
					</ContextMenu.Item>
					<ContextMenu.Separator />
					<ContextMenu.Item onclick={() => reveal(tab.id)}>Reveal in sidebar</ContextMenu.Item>
				</ContextMenu.Content>
			</ContextMenu.Root>
		{/each}
	</div>
	{#if overflowing}<TabListMenu onpick={show} />{/if}
</nav>
