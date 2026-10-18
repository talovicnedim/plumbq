<script lang="ts">
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import AskAi from '#lib/components/ai/AskAi.svelte';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import { bodyModes } from '#lib/state/body-modes.svelte.js';
	import { settings } from '#lib/state/settings.svelte.js';
	import type { RequestTab } from '#lib/state/tabs.svelte.js';
	import { ASK } from '#lib/utils/ai.js';
	import { summarize } from '#lib/utils/assertions.js';
	import { headerValue } from '#lib/utils/http.js';
	import { previewKind, type BodyMode } from '#lib/utils/preview.js';
	import { labelFor } from '#lib/utils/shortcuts.js';
	import CompareDialog from './CompareDialog.svelte';
	import CookiesTab from './CookiesTab.svelte';
	import HeadersTable from './HeadersTable.svelte';
	import ResponseActions from './ResponseActions.svelte';
	import ResponseBody from './ResponseBody.svelte';
	import ResponseStatusBar from './ResponseStatusBar.svelte';
	import TestResults from './TestResults.svelte';

	interface Props {
		tab: RequestTab;
	}

	let { tab }: Props = $props();

	let wrap = $state(settings.values.wordWrap);
	let comparing = $state(false);

	const response = $derived(tab.entry?.response);
	const contentType = $derived(
		response ? headerValue(response.headers, 'content-type') : undefined
	);
	const tests = $derived(summarize(response?.assertions));
	const chosenMode = $derived(bodyModes.get(contentType));
	const canPreview = $derived(previewKind(contentType) !== null);
	// Binary bodies open as a preview; text (including HTML) as pretty-printed source.
	const mode = $derived<BodyMode>(
		chosenMode === 'preview' && !canPreview
			? 'pretty'
			: (chosenMode ?? (response?.binary && canPreview ? 'preview' : 'pretty'))
	);
	const modes = $derived<BodyMode[]>(canPreview ? ['pretty', 'raw', 'preview'] : ['pretty', 'raw']);
</script>

<section class="flex h-full min-h-0 flex-col">
	<ResponseStatusBar {tab} />

	{#if tab.error && !tab.sending}
		<div
			class="m-4 flex gap-3 rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm"
		>
			<CircleAlertIcon class="mt-0.5 size-4 shrink-0 text-destructive" />
			<div class="grid gap-1">
				<p class="font-medium">Couldn't send the request</p>
				<p class="font-mono text-xs break-all text-muted-foreground select-text">{tab.error}</p>
				<div>
					<AskAi
						label="Why did this fail?"
						prompt={ASK.sendFailed(tab.id, tab.error)}
						attach={{ request: true, response: false }}
						class="-ml-1.5"
					/>
				</div>
			</div>
		</div>
	{:else if response}
		<Tabs.Root value={tests.allPassed ? 'body' : 'tests'} class="min-h-0 flex-1 gap-3 p-4 pt-3">
			<div class="flex items-center justify-between">
				<Tabs.List class="h-8">
					<Tabs.Trigger value="body" class="text-xs">Body</Tabs.Trigger>
					<Tabs.Trigger value="headers" class="text-xs">
						Headers <span class="text-muted-foreground">{response.headers.length}</span>
					</Tabs.Trigger>
					<Tabs.Trigger value="tests" class="text-xs">
						Tests
						{#if tests.total > 0}
							<span class={tests.allPassed ? 'text-muted-foreground' : 'text-destructive'}>
								{tests.passed}/{tests.total}
							</span>
						{/if}
					</Tabs.Trigger>
					<Tabs.Trigger value="sent" class="text-xs">Request sent</Tabs.Trigger>
					<Tabs.Trigger value="cookies" class="text-xs">Cookies</Tabs.Trigger>
				</Tabs.List>
				<ResponseActions
					requestId={tab.id}
					{response}
					{contentType}
					{mode}
					{modes}
					bind:wrap
					oncompare={() => (comparing = true)}
				/>
			</div>
			<Tabs.Content value="body" class="min-h-0">
				<ResponseBody {response} {contentType} {mode} {wrap} key={tab.entry?.id ?? ''} />
			</Tabs.Content>
			<Tabs.Content value="cookies" class="min-h-0 overflow-y-auto">
				<CookiesTab reloadKey={tab.entry?.id} />
			</Tabs.Content>
			<Tabs.Content value="tests" class="min-h-0 overflow-y-auto">
				<TestResults results={response.assertions ?? []} requestId={tab.id} />
			</Tabs.Content>
			<Tabs.Content value="headers" class="min-h-0 overflow-y-auto">
				<HeadersTable headers={response.headers} />
			</Tabs.Content>
			<Tabs.Content
				value="sent"
				class="grid min-h-0 content-start gap-3 overflow-y-auto font-mono text-xs select-text"
			>
				<p class="break-all">
					<span class="font-semibold">{response.sent.method}</span>
					{response.sent.url}
				</p>
				<HeadersTable headers={response.sent.headers} />
				{#if response.sent.body}<pre class="whitespace-pre-wrap">{response.sent.body}</pre>{/if}
				<p class="font-sans text-muted-foreground">
					Secret values are shown as their variable name.
				</p>
			</Tabs.Content>
		</Tabs.Root>
	{:else if tab.cancelled && !tab.sending}
		<p class="py-12 text-center text-sm text-muted-foreground">
			Request cancelled. Send it again with {labelFor('send')}.
		</p>
	{:else if !tab.sending}
		<div
			class="flex flex-1 flex-col items-center justify-center gap-3 text-sm text-muted-foreground"
		>
			<p>Send the request to see the response.</p>
			<div class="flex gap-4 text-xs">
				<span><kbd class="rounded border px-1.5 py-0.5">{labelFor('send')}</kbd> send</span>
				<span><kbd class="rounded border px-1.5 py-0.5">{labelFor('save')}</kbd> save</span>
				<span><kbd class="rounded border px-1.5 py-0.5">{labelFor('palette')}</kbd> search</span>
			</div>
		</div>
	{/if}
</section>

<CompareDialog bind:open={comparing} {tab} />
