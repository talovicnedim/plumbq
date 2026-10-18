<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { tabs, type RequestTab } from '#lib/state/tabs.svelte.js';
	import { labelFor } from '#lib/utils/shortcuts.js';
	import CopyAsMenu from './CopyAsMenu.svelte';

	interface Props {
		tab: RequestTab;
	}

	let { tab }: Props = $props();

	const dirty = $derived(tabs.isDirty(tab));
</script>

<div class="flex items-center gap-2">
	<input
		class="min-w-0 flex-1 bg-transparent text-[15px] font-semibold outline-none placeholder:text-muted-foreground"
		bind:value={tab.draft.name}
		placeholder="Untitled request"
		spellcheck="false"
	/>
	<span class="truncate font-mono text-xs text-muted-foreground" title="File in the workspace">
		{tab.id}
	</span>
	<CopyAsMenu {tab} />
	<Button
		size="sm"
		variant={dirty ? 'default' : 'outline'}
		class="h-7"
		disabled={!dirty}
		onclick={() => tabs.save(tab)}
		title={`Save to the .http file (${labelFor('save')})`}
	>
		{dirty ? 'Save' : 'Saved'}
	</Button>
</div>
