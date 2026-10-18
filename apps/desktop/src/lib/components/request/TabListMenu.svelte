<script lang="ts">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { tabs } from '#lib/state/tabs.svelte.js';
	import { cn } from '#lib/utils.js';
	import { methodColor } from '#lib/utils/http.js';

	interface Props {
		/** Shows the picked tab, scrolling it into view. */
		onpick: (id: string) => void;
	}

	let { onpick }: Props = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class="flex shrink-0 items-center gap-1 border-l px-2.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground data-[state=open]:bg-muted data-[state=open]:text-foreground"
		aria-label="All open tabs"
		title="All open tabs"
	>
		<ChevronDownIcon class="size-3.5" />
		<span class="tabular-nums">{tabs.all.length}</span>
	</DropdownMenu.Trigger>
	<!-- pr-3: macOS draws the scrollbar over the right edge, which would cover the unsaved dots. -->
	<DropdownMenu.Content align="end" class="max-h-[60vh] w-72 overflow-y-auto pr-3">
		{#each tabs.all as tab (tab.id)}
			<DropdownMenu.Item class="gap-2" onclick={() => onpick(tab.id)}>
				<span
					class={cn(
						'w-12 shrink-0 font-mono text-[10px] font-semibold',
						methodColor(tab.draft.method)
					)}
				>
					{tab.draft.method}
				</span>
				<span class={cn('truncate', tab.id === tabs.activeId && 'font-medium')}>
					{tab.draft.name || tab.id}
				</span>
				{#if tabs.isDirty(tab)}
					<span class="ml-auto size-1.5 shrink-0 rounded-full bg-foreground/60" title="Unsaved"
					></span>
				{/if}
			</DropdownMenu.Item>
		{/each}
	</DropdownMenu.Content>
</DropdownMenu.Root>
