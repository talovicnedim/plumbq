<script lang="ts">
	import { labelFor } from '#lib/utils/shortcuts.js';
	import CheckIcon from '@lucide/svelte/icons/check';
	import ChevronsUpDownIcon from '@lucide/svelte/icons/chevrons-up-down';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import FolderPlusIcon from '@lucide/svelte/icons/folder-plus';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { workspace } from '#lib/state/workspace.svelte.js';

	interface Props {
		open?: boolean;
	}

	let { open = $bindable(false) }: Props = $props();

	const initial = $derived(([...workspace.name][0] ?? '?').toUpperCase());
</script>

<DropdownMenu.Root bind:open>
	<DropdownMenu.Trigger
		class="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-md px-1.5 text-left hover:bg-sidebar-foreground/6 data-[state=open]:bg-sidebar-foreground/6"
		title={`${workspace.root}\nSwitch workspace (${labelFor('switchWorkspace')})`}
	>
		<span
			class="flex size-6 shrink-0 items-center justify-center rounded-md bg-primary/15 text-[12px] font-semibold text-primary"
			aria-hidden="true"
		>
			{initial}
		</span>
		<span class="truncate text-[15px] font-semibold">{workspace.name}</span>
		<ChevronsUpDownIcon class="ml-auto size-4 shrink-0 text-muted-foreground" />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="start" class="w-72">
		<DropdownMenu.Label class="text-xs text-muted-foreground">Recent workspaces</DropdownMenu.Label>
		{#each workspace.recent as recent (recent.root)}
			<DropdownMenu.Item onclick={() => workspace.open(recent.root)} class="items-start gap-2">
				<CheckIcon class={['mt-0.5 size-3.5', recent.root !== workspace.root && 'invisible']} />
				<span class="grid min-w-0">
					<span class="truncate">{recent.name}</span>
					<span class="truncate text-xs text-muted-foreground">{recent.root}</span>
				</span>
			</DropdownMenu.Item>
		{/each}
		<DropdownMenu.Separator />
		<DropdownMenu.Item onclick={() => workspace.rename()}>
			<PencilIcon /> Rename workspace…
		</DropdownMenu.Item>
		<DropdownMenu.Item onclick={() => workspace.create()}>
			<FolderPlusIcon /> New workspace…
		</DropdownMenu.Item>
		<DropdownMenu.Item onclick={() => workspace.choose()}>
			<FolderOpenIcon /> Open folder… <DropdownMenu.Shortcut
				>{labelFor('openWorkspace')}</DropdownMenu.Shortcut
			>
		</DropdownMenu.Item>
	</DropdownMenu.Content>
</DropdownMenu.Root>
