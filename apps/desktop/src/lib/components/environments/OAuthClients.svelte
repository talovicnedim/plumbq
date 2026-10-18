<script lang="ts">
	import PlusIcon from '@lucide/svelte/icons/plus';
	import XIcon from '@lucide/svelte/icons/x';
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type OAuthClient } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { prompt } from '#lib/state/prompt.svelte.js';

	interface Props {
		/** The environment's clients being edited; saved with the environment. */
		clients: OAuthClient[];
		environment: string;
		/** Saves the environment, so sign-in uses what is on screen. */
		onsave: () => Promise<unknown>;
	}

	let { clients = $bindable(), environment, onsave }: Props = $props();

	let busy = $state<string | null>(null);

	async function add() {
		const name = await prompt({
			title: 'New OAuth 2.0 client',
			label: 'Name, used as {{$auth.token("name")}}',
			confirmLabel: 'Add',
			placeholder: 'github'
		});
		if (!name) return;
		clients = [...clients, await api.auth.client.new(name)];
	}

	async function signIn(client: OAuthClient) {
		busy = client.name;
		const waiting = toast.loading(`Waiting for sign-in to ${client.name} in the browser…`);
		try {
			await onsave();
			await api.auth.signIn(environment, client.name);
			toast.success(`Signed in to ${client.name}`, { id: waiting });
			await environments.list.refresh();
		} catch (err) {
			toast.error(toApiError(err).message, { id: waiting });
		} finally {
			busy = null;
		}
	}

	async function signOut(client: OAuthClient) {
		try {
			await api.auth.signOut(environment, client.name);
			await environments.list.refresh();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}
</script>

<div class="grid gap-2 text-sm">
	OAuth 2.0
	{#each clients as client, i (client.name)}
		<div class="grid gap-2 rounded-md border p-3">
			<div class="flex items-center gap-2">
				<code class="text-xs font-semibold">{client.name}</code>
				<select
					class="rounded border bg-transparent px-1.5 py-0.5 text-xs"
					bind:value={client.grant}
				>
					<option value="authorization_code">Sign in (authorization code + PKCE)</option>
					<option value="client_credentials">Client credentials</option>
				</select>
				<span class="ml-auto text-xs text-muted-foreground">
					{client.signedIn ? 'token cached' : 'no token yet'}
				</span>
				{#if client.grant === 'authorization_code'}
					{#if client.signedIn}
						<Button size="sm" variant="ghost" class="h-7" onclick={() => signOut(client)}
							>Sign out</Button
						>
					{/if}
					<Button
						size="sm"
						variant="outline"
						class="h-7"
						disabled={busy !== null || !client.authUrl || !client.tokenUrl}
						onclick={() => signIn(client)}
					>
						{busy === client.name ? 'Waiting…' : 'Sign in'}
					</Button>
				{:else if client.signedIn}
					<Button size="sm" variant="ghost" class="h-7" onclick={() => signOut(client)}
						>Forget token</Button
					>
				{/if}
				<button
					type="button"
					class="text-muted-foreground hover:text-foreground"
					aria-label="Remove client"
					onclick={() => (clients = clients.filter((_, j) => j !== i))}
				>
					<XIcon class="size-3.5" />
				</button>
			</div>
			<div class="grid grid-cols-2 gap-2">
				{#if client.grant === 'authorization_code'}
					<Input
						class="h-8 font-mono text-xs"
						bind:value={client.authUrl}
						placeholder="Authorization URL"
					/>
				{/if}
				<Input class="h-8 font-mono text-xs" bind:value={client.tokenUrl} placeholder="Token URL" />
				<Input class="h-8 font-mono text-xs" bind:value={client.clientId} placeholder="Client ID" />
				<Input
					class="h-8 font-mono text-xs"
					bind:value={client.clientSecret}
					placeholder={'Client secret, e.g. {{clientSecret}}'}
				/>
				<Input class="h-8 font-mono text-xs" bind:value={client.scope} placeholder="Scope" />
				{#if client.grant === 'authorization_code'}
					<Input
						class="h-8 font-mono text-xs"
						bind:value={client.redirectUrl}
						placeholder={`Redirect URL (default ${client.defaultRedirectUrl})`}
					/>
				{/if}
			</div>
		</div>
	{/each}
	<Button size="sm" variant="ghost" class="justify-start" onclick={add}>
		<PlusIcon /> Add OAuth client
	</Button>
	<p class="text-xs text-muted-foreground">
		Use <code>{'{{$auth.token("name")}}'}</code> in a header, or pick OAuth 2.0 in a request's Auth tab.
		Put the client secret in a keychain secret and reference it. Tokens are cached in the keychain and
		refreshed automatically.
	</p>
</div>
