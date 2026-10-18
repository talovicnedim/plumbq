<script lang="ts">
	import { api, type McpClient } from '#lib/api/index.js';
	import CopyField from '#lib/components/common/CopyField.svelte';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import { Query } from '#lib/state/query.svelte.js';
	import ClientTabs from './ClientTabs.svelte';
	import DevBuildHint from './DevBuildHint.svelte';
	import OneClickRow from './OneClickRow.svelte';
	import ServerSettings from './ServerSettings.svelte';

	const apps = new Query(() => api.mcp.apps.list());
	const clients = new Query(() => api.mcp.clients.list());

	let client = $state<McpClient>('claudeCode');

	const settings = $derived(ai.status?.settings);
	const url = $derived(ai.status?.url);
	const tabs = $derived(
		(clients.data ?? [])
			.filter((c) => c.client !== 'stdio')
			.map((c) => ({ value: c.client, label: c.label }))
	);
	const setup = $derived(clients.data?.find((c) => c.client === client));
	const stdio = $derived(clients.data?.find((c) => c.client === 'stdio'));

	// The URL changes with the port and token, which makes existing entries out of date.
	$effect(() => {
		void url;
		void apps.refresh();
		void clients.refresh();
	});
</script>

{#if settings}
	<div class="grid gap-4 text-sm">
		<p class="text-muted-foreground">
			Let Claude, ChatGPT, Cursor, Codex or Copilot organize, write and send the requests in this
			workspace. Anything that changes a request or sends one that changes data pops up in Plumbq
			for your approval first.
		</p>

		{#if ai.status?.settingsError}
			<p
				class="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive"
			>
				{ai.status.settingsError}. AI apps and the chat are refused until you change a setting here,
				which saves the settings again.
			</p>
		{/if}

		{#if !settings.enabled}
			<div class="flex items-center justify-between gap-6 rounded-xl border p-5">
				<div>
					<p class="font-medium">Turn on the MCP server</p>
					<p class="text-muted-foreground">
						It runs only on this computer (127.0.0.1) while Plumbq is open, on the workspace open
						here.
					</p>
				</div>
				<Button onclick={() => ai.update({ enabled: true })}>Turn on</Button>
			</div>
		{:else}
			{#if ai.status?.error}
				<p
					class="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive"
				>
					{ai.status.error}. Pick another port under Server settings.
				</p>
			{/if}

			{#if url && setup}
				<ClientTabs clients={tabs} bind:value={client} />
				{#if setup.oneClick}
					{@const app = setup.oneClick}
					<QueryView query={apps}>
						{#snippet children(list)}
							{@const status = list.find((a) => a.app === app)}
							{#if status}<OneClickRow {status} onchange={() => apps.refresh()} />{/if}
						{/snippet}
					</QueryView>
					<p class="text-xs text-muted-foreground">Or set it up yourself. {setup.hint}</p>
				{:else}
					<p class="font-medium">{setup.hint}</p>
				{/if}
				<CopyField value={setup.snippet} multiline />
				{#if setup.note}<p class="text-xs text-muted-foreground">{setup.note}</p>{/if}
				{#if setup.usesBinary}<DevBuildHint />{/if}
			{/if}

			<ServerSettings {stdio} />
		{/if}
	</div>
{/if}
