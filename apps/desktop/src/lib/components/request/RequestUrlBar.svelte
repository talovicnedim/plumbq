<script lang="ts">
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import SendIcon from '@lucide/svelte/icons/send-horizontal';
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type VariableHint } from '#lib/api/index.js';
	import VariableInput from '#lib/components/common/VariableInput.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { toastCurlSecrets } from '#lib/state/curl-secrets.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { tabs, type RequestTab } from '#lib/state/tabs.svelte.js';
	import { labelFor } from '#lib/utils/shortcuts.js';
	import MethodInput from './MethodInput.svelte';

	interface Props {
		tab: RequestTab;
		/** How each `{{variable}}` resolves, from core. */
		variables: VariableHint[];
	}

	let { tab, variables }: Props = $props();

	/** Pasting a whole curl command into the URL bar turns it into this request. */
	async function onpaste(e: ClipboardEvent) {
		const text = e.clipboardData?.getData('text/plain') ?? '';
		if (!/^\s*curl\s/.test(text)) return;
		e.preventDefault();
		try {
			const { request, secrets } = await api.curl.parse(text, environments.selected);
			tab.draft = {
				...tab.draft,
				method: request.method,
				url: request.url,
				headers: request.headers,
				body: request.body
			};
			toast.success('Imported from cURL');
			toastCurlSecrets(secrets, environments.selected);
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}
</script>

<form
	class="flex h-10 shrink-0 items-stretch overflow-hidden rounded-lg border bg-background shadow-xs focus-within:ring-2 focus-within:ring-ring/40"
	onsubmit={(e) => {
		e.preventDefault();
		void tabs.send(tab);
	}}
	onpastecapture={onpaste}
>
	<MethodInput value={tab.draft.method} onchange={(method) => (tab.draft.method = method)} />
	<VariableInput
		bind:value={tab.draft.url}
		{variables}
		placeholder={'{{baseUrl}}/users, or paste a curl command'}
		class="flex-1"
	/>
	{#if tab.sending}
		<Button
			type="button"
			variant="secondary"
			class="h-auto rounded-none px-5"
			title="Cancel (Esc)"
			onclick={() => tabs.cancel(tab)}
		>
			<LoaderIcon class="animate-spin" />
			Cancel
		</Button>
	{:else}
		<Button
			type="submit"
			class="h-auto rounded-none px-5"
			disabled={!tab.draft.url}
			title={`Send (${labelFor('send')})`}
		>
			<SendIcon />
			Send
		</Button>
	{/if}
</form>
