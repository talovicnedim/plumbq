<script lang="ts" module>
	const HEADER_NAMES = [
		'Accept',
		'Accept-Encoding',
		'Accept-Language',
		'Authorization',
		'Cache-Control',
		'Content-Type',
		'Cookie',
		'If-Match',
		'If-None-Match',
		'Origin',
		'Referer',
		'User-Agent',
		'X-API-Key',
		'X-Request-ID'
	];
</script>

<script lang="ts">
	import type { RequestView } from '#lib/api/index.js';
	import AskAi from '#lib/components/ai/AskAi.svelte';
	import KeyValueEditor from '#lib/components/common/KeyValueEditor.svelte';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import type { RequestTab } from '#lib/state/tabs.svelte.js';
	import { ASK } from '#lib/utils/ai.js';
	import { authFromHeaders } from '#lib/utils/auth.js';
	import { parseQuery, withQuery } from '#lib/utils/http.js';
	import { hasRequestSettings } from '#lib/utils/request-settings.js';
	import AuthEditor from './AuthEditor.svelte';
	import BodyEditor from './BodyEditor.svelte';
	import DocsEditor from './DocsEditor.svelte';
	import RequestSettingsEditor from './RequestSettings.svelte';

	interface Props {
		tab: RequestTab;
		/** What core says about the draft; undefined until it answers. */
		view: RequestView | undefined;
	}

	let { tab, view }: Props = $props();

	const params = $derived(parseQuery(tab.draft.url));
	const bodyWarnings = $derived(view?.bodyWarnings ?? []);
	const hasDocs = $derived(!!view?.docs);
	const hasSettings = $derived(
		(tab.draft.variables?.length ?? 0) > 0 || hasRequestSettings(tab.draft.settings)
	);
	const auth = $derived(authFromHeaders(tab.draft.headers));
</script>

<Tabs.Root value="params" class="min-h-0 flex-1 gap-3">
	<Tabs.List class="h-8">
		<Tabs.Trigger value="params" class="text-xs">
			Params {#if params.length}<span class="text-muted-foreground">{params.length}</span>{/if}
		</Tabs.Trigger>
		<Tabs.Trigger value="headers" class="text-xs">
			Headers
			{#if tab.draft.headers.length}
				<span class="text-muted-foreground">{tab.draft.headers.length}</span>
			{/if}
		</Tabs.Trigger>
		<Tabs.Trigger value="body" class="text-xs">
			Body {#if tab.draft.body}<span class="size-1.5 rounded-full bg-primary"></span>{/if}
		</Tabs.Trigger>
		<Tabs.Trigger value="auth" class="text-xs">
			Auth {#if auth.kind !== 'none'}<span class="size-1.5 rounded-full bg-primary"></span>{/if}
		</Tabs.Trigger>
		<Tabs.Trigger value="assertions" class="text-xs">
			Assertions
			{#if tab.draft.assertions.length}
				<span class="text-muted-foreground">{tab.draft.assertions.length}</span>
			{/if}
		</Tabs.Trigger>
		<Tabs.Trigger value="docs" class="text-xs">
			Docs {#if hasDocs}<span class="size-1.5 rounded-full bg-primary"></span>{/if}
		</Tabs.Trigger>
		<Tabs.Trigger value="settings" class="text-xs">
			Settings
			{#if hasSettings}<span class="size-1.5 rounded-full bg-primary"></span>{/if}
		</Tabs.Trigger>
	</Tabs.List>
	<Tabs.Content value="params" class="min-h-0 overflow-y-auto">
		<KeyValueEditor
			rows={params}
			onchange={(rows) => (tab.draft.url = withQuery(tab.draft.url, rows))}
			namePlaceholder="parameter"
			bulk="="
			variables={view?.variables ?? []}
		/>
	</Tabs.Content>
	<Tabs.Content value="headers" class="min-h-0 overflow-y-auto">
		<KeyValueEditor
			rows={tab.draft.headers}
			onchange={(rows) => (tab.draft.headers = rows)}
			namePlaceholder="Header"
			suggestions={HEADER_NAMES}
			bulk=":"
			variables={view?.variables ?? []}
		/>
	</Tabs.Content>
	<Tabs.Content value="body" class="flex min-h-0 flex-col gap-2">
		{#each bodyWarnings as warning, i (i)}
			<p class="rounded-md border border-warning/40 bg-warning/10 px-2 py-1 text-xs">
				{warning}
			</p>
		{/each}
		<div class="min-h-0 flex-1">
			<BodyEditor
				body={tab.draft.body}
				headers={tab.draft.headers}
				onbody={(body) => (tab.draft.body = body)}
				onheaders={(headers) => (tab.draft.headers = headers)}
				key={tab.id}
				fileReference={view?.fileReference}
				variables={view?.variables ?? []}
			/>
		</div>
	</Tabs.Content>
	<Tabs.Content value="auth" class="min-h-0 overflow-y-auto">
		<AuthEditor headers={tab.draft.headers} onchange={(headers) => (tab.draft.headers = headers)} />
	</Tabs.Content>
	<Tabs.Content value="assertions" class="grid min-h-0 content-start gap-2 overflow-y-auto">
		<p class="text-xs text-muted-foreground">
			Checked on every send; results show in the response's Tests tab. Examples:
			<code>status == 200</code>, <code>duration &lt; 500</code>,
			<code>header Content-Type contains json</code>, <code>body.$.items.length &gt; 0</code>,
			<code>body.$.user.name == "Ada"</code>.
		</p>
		<div>
			<AskAi
				label="Write tests for this"
				prompt={ASK.writeTests(tab.id)}
				attach={{ request: true, response: true }}
			/>
		</div>
		<KeyValueEditor
			rows={tab.draft.assertions.map((a) => ({ name: a, value: '' }))}
			onchange={(rows) => (tab.draft.assertions = rows.map((r) => r.name).filter(Boolean))}
			namePlaceholder="status in 200..299"
			valuePlaceholder=""
		/>
	</Tabs.Content>
	<Tabs.Content value="docs" class="flex min-h-0 flex-col gap-2">
		<div>
			<AskAi
				label="Document this"
				prompt={ASK.document(tab.id)}
				attach={{ request: true, response: true }}
			/>
		</div>
		<div class="min-h-0 flex-1">
			<DocsEditor
				docs={view?.docs ?? ''}
				comments={tab.draft.comments}
				oncomments={(c) => (tab.draft.comments = c)}
			/>
		</div>
	</Tabs.Content>
	<Tabs.Content value="settings" class="min-h-0 overflow-y-auto">
		<RequestSettingsEditor
			settings={tab.draft.settings ?? {}}
			variables={tab.draft.variables ?? []}
			onsettings={(settings) => (tab.draft.settings = settings)}
			onvariables={(variables) => (tab.draft.variables = variables)}
		/>
	</Tabs.Content>
</Tabs.Root>
