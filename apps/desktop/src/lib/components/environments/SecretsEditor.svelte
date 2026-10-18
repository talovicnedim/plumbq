<script lang="ts">
	import PlusIcon from '@lucide/svelte/icons/plus';
	import { api, type SecretVariable } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { confirm } from '#lib/state/confirm.svelte.js';
	import { prompt } from '#lib/state/prompt.svelte.js';

	interface Props {
		environment: string;
		secrets: SecretVariable[];
		/** Runs a change, then refreshes the environments and shows `success`. */
		run: (change: () => Promise<unknown>, success?: string) => Promise<void>;
	}

	let { environment, secrets = $bindable(), run }: Props = $props();

	/** The only way a secret goes: saving the environment never removes one. */
	async function removeSecret(name: string) {
		const ok = await confirm({
			title: `Remove secret "${name}"?`,
			description: `It's removed from ${environment} and its value is deleted from this machine's keychain.`,
			confirmLabel: 'Remove',
			destructive: true
		});
		if (!ok) return;
		secrets = secrets.filter((s) => s.name !== name);
		await run(() => api.environments.removeSecret(environment, name), 'Secret removed');
	}

	async function setSecret(name?: string) {
		const secretName =
			name ??
			(await prompt({
				title: 'New secret',
				label: 'Name',
				confirmLabel: 'Next',
				placeholder: 'token'
			}));
		if (!secretName) return;
		const value = await prompt({
			title: `Secret "${secretName}"`,
			label: 'Value (stored in the OS keychain, never in your files)',
			confirmLabel: 'Save'
		});
		if (!value) return;
		// The new name goes into the draft too, so it isn't mistaken for a change made elsewhere.
		if (!secrets.some((s) => s.name === secretName)) {
			secrets = [...secrets, { name: secretName, isSet: true }];
		}
		await run(() => api.environments.setSecret(environment, secretName, value), 'Secret saved');
	}
</script>

<div class="grid gap-1.5 text-sm">
	Secrets
	{#each secrets as secret (secret.name)}
		<div class="flex items-center justify-between rounded-md border px-2 py-1.5">
			<code class="text-xs">{secret.name}</code>
			<span class="flex items-center gap-2 text-xs text-muted-foreground">
				{secret.isSet ? 'set on this machine' : 'not set'}
				<Button size="sm" variant="outline" onclick={() => setSecret(secret.name)}>
					{secret.isSet ? 'Change' : 'Set'}
				</Button>
				<Button
					size="sm"
					variant="ghost"
					class="text-destructive"
					onclick={() => removeSecret(secret.name)}
				>
					Remove
				</Button>
			</span>
		</div>
	{/each}
	<Button size="sm" variant="ghost" class="justify-start" onclick={() => setSecret()}>
		<PlusIcon /> Add secret
	</Button>
</div>
