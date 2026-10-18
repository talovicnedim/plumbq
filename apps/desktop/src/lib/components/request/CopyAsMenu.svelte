<script lang="ts">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import CodeIcon from '@lucide/svelte/icons/code';
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type CodeLanguage } from '#lib/api/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import type { RequestTab } from '#lib/state/tabs.svelte.js';
	import { CODE_LANGUAGES } from '#lib/utils/codegen.js';

	interface Props {
		tab: RequestTab;
	}

	let { tab }: Props = $props();

	async function copy(language: CodeLanguage, label: string) {
		try {
			const code = await api.requests.export(tab.id, tab.draft, environments.selected, language);
			await navigator.clipboard.writeText(code);
			toast.success(`Copied as ${label}`, { description: 'Secrets are left as {{name}}.' });
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class="flex h-7 items-center gap-1.5 rounded-md px-2.5 text-xs hover:bg-muted"
		title="Copy as cURL or code"
	>
		<CodeIcon class="size-3.5" /> Copy as <ChevronDownIcon class="size-3 opacity-60" />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="w-52">
		{#each CODE_LANGUAGES as language (language.id)}
			<DropdownMenu.Item onclick={() => copy(language.id, language.label)}>
				{language.label}
			</DropdownMenu.Item>
		{/each}
	</DropdownMenu.Content>
</DropdownMenu.Root>
