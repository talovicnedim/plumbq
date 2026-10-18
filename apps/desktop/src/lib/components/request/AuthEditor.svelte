<script lang="ts">
	import type { Header } from '#lib/api/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { cn } from '#lib/utils.js';
	import {
		AUTH_KINDS,
		AUTH_LABEL,
		authFromHeaders,
		withAuth,
		type Auth,
		type AuthKind
	} from '#lib/utils/auth.js';

	interface Props {
		headers: Header[];
		onchange: (headers: Header[]) => void;
	}

	let { headers, onchange }: Props = $props();

	const auth = $derived(authFromHeaders(headers));

	function set(next: Partial<Auth>) {
		onchange(withAuth(headers, { ...auth, ...next }));
	}

	const clients = $derived(environments.current?.auth ?? []);

	function setKind(kind: AuthKind) {
		const header = kind === 'apiKey' ? auth.header || 'X-API-Key' : '';
		const value = kind === 'oauth2' ? (clients[0]?.name ?? 'oauth') : auth.value;
		onchange(withAuth(headers, { kind, value, header }));
	}
</script>

<div class="grid max-w-xl gap-4">
	<div class="inline-flex w-fit rounded-md border p-0.5">
		{#each AUTH_KINDS as kind (kind)}
			<button
				type="button"
				class={cn(
					'rounded px-2.5 py-1 text-xs',
					auth.kind === kind
						? 'bg-muted font-medium'
						: 'text-muted-foreground hover:text-foreground'
				)}
				onclick={() => setKind(kind)}
			>
				{AUTH_LABEL[kind]}
			</button>
		{/each}
	</div>

	{#if auth.kind === 'bearer'}
		<label class="grid gap-1.5 text-sm">
			Token
			<Input
				class="font-mono text-xs"
				value={auth.value}
				placeholder={'{{token}}'}
				oninput={(e) => set({ value: e.currentTarget.value })}
			/>
		</label>
	{:else if auth.kind === 'basic'}
		<label class="grid gap-1.5 text-sm">
			Username and password
			<Input
				class="font-mono text-xs"
				value={auth.value}
				placeholder={'{{user}}:{{password}}'}
				oninput={(e) => set({ value: e.currentTarget.value })}
			/>
		</label>
	{:else if auth.kind === 'apiKey'}
		<div class="grid grid-cols-[1fr_2fr] gap-2">
			<label class="grid gap-1.5 text-sm">
				Header
				<Input
					class="font-mono text-xs"
					value={auth.header}
					oninput={(e) => set({ header: e.currentTarget.value })}
				/>
			</label>
			<label class="grid gap-1.5 text-sm">
				Value
				<Input
					class="font-mono text-xs"
					value={auth.value}
					placeholder={'{{apiKey}}'}
					oninput={(e) => set({ value: e.currentTarget.value })}
				/>
			</label>
		</div>
	{:else if auth.kind === 'oauth2'}
		<label class="grid gap-1.5 text-sm">
			OAuth client
			{#if clients.length > 0}
				<select
					class="rounded-md border bg-transparent px-2 py-1.5 text-sm"
					value={auth.value}
					onchange={(e) => set({ value: e.currentTarget.value })}
				>
					{#each clients as client (client.name)}
						<option value={client.name}>
							{client.name} ({client.grant === 'client_credentials'
								? 'client credentials'
								: 'sign-in'})
						</option>
					{/each}
					{#if !clients.some((c) => c.name === auth.value)}
						<option value={auth.value}>{auth.value} (not in this environment)</option>
					{/if}
				</select>
			{:else}
				<span class="text-xs text-muted-foreground">
					{environments.current
						? `No OAuth clients in ${environments.current.name} yet. Add one in Manage environments.`
						: 'Pick an environment with an OAuth client first.'}
				</span>
			{/if}
		</label>
	{/if}

	<p class="text-xs text-muted-foreground">
		{#if auth.kind === 'none'}
			No auth header. Folder headers from <code>folder.toml</code> still apply.
		{:else if auth.kind === 'oauth2'}
			Saved as <code>Bearer {'{{$auth.token("' + auth.value + '")}}'}</code>. The token is fetched,
			cached in the keychain and refreshed for you.
		{:else}
			Saved as a header in the <code>.http</code> file. Keep credentials in an environment secret,
			e.g. <code>{'{{token}}'}</code>, so they never end up in git.
		{/if}
	</p>
</div>
