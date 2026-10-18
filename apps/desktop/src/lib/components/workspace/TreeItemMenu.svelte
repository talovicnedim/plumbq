<script lang="ts">
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import type { nodeMenu } from './menu.js';

	interface Props {
		menu: ReturnType<typeof nodeMenu>;
	}

	let { menu }: Props = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class="absolute top-1 right-1 flex size-6 items-center justify-center rounded-md text-muted-foreground opacity-0 group-hover:opacity-100 group-has-focus-visible:opacity-100 hover:bg-background hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none data-[state=open]:bg-background data-[state=open]:text-foreground data-[state=open]:opacity-100"
		aria-label="Actions"
	>
		<EllipsisIcon class="size-4" />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="start" class="w-48">
		{#each menu as entry (entry.label)}
			{#if entry.separated}<DropdownMenu.Separator />{/if}
			<DropdownMenu.Item
				variant={entry.destructive ? 'destructive' : 'default'}
				onclick={() => entry.run()}
			>
				{entry.label}
			</DropdownMenu.Item>
		{/each}
	</DropdownMenu.Content>
</DropdownMenu.Root>
