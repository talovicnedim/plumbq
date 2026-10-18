<script lang="ts">
	import CheckIcon from '@lucide/svelte/icons/check';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import { untrack } from 'svelte';
	import { api, toApiError, type AssistantSettings, type ProviderConfig } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
	import { cn } from '#lib/utils.js';
	import { providerMeta } from '#lib/utils/providers.js';
	import { labelFor } from '#lib/utils/shortcuts.js';
	import ApiKeyField from './ApiKeyField.svelte';
	import ProviderAdvanced from './ProviderAdvanced.svelte';
	import ProviderTiles from './ProviderTiles.svelte';

	interface Check {
		state: 'idle' | 'busy' | 'ok' | 'error';
		message: string;
	}

	let key = $state('');
	let models = $state<string[]>([]);
	let check = $state<Check>({ state: 'idle', message: '' });

	const settings = $derived(assistant.status?.settings);
	const active = $derived(assistant.provider);
	// Every status update is a new provider object; only a different id is another provider.
	const activeId = $derived(active?.id);
	const meta = $derived(providerMeta(active?.kind));
	const hasKey = $derived(assistant.providerStatus?.hasKey ?? false);
	const needsKey = $derived(assistant.providerStatus?.needsKey ?? false);

	// Another provider picked: fetch its models (local ones always, cloud ones once keyed).
	$effect(() => {
		if (!activeId) return;
		untrack(() => {
			key = '';
			models = [];
			check = { state: 'idle', message: '' };
			if (!needsKey || hasKey) void loadModels();
		});
	});

	async function save(change: Partial<AssistantSettings>) {
		if (!settings) return;
		try {
			assistant.setStatus(await api.assistant.update({ ...settings, ...change }));
		} catch (err) {
			check = { state: 'error', message: toApiError(err).message };
		}
	}

	function update(change: Partial<ProviderConfig>) {
		if (!settings || !active) return;
		const id = active.id;
		void save({
			providers: settings.providers.map((p) => (p.id === id ? { ...p, ...change } : p))
		});
	}

	async function loadModels() {
		const provider = active;
		if (!provider) return;
		try {
			models = await api.assistant.models(provider.id);
			if (!provider.model && models.length > 0) update({ model: models[0] });
			check = { state: 'ok', message: `${models.length} models available` };
		} catch (err) {
			models = [];
			check = {
				state: 'error',
				message: `couldn't reach the provider: ${toApiError(err).message}`
			};
		}
	}

	async function saveKey() {
		if (!active || !key.trim()) return;
		check = { state: 'busy', message: 'Checking the key...' };
		try {
			assistant.setStatus(await api.assistant.key.set(active.id, key.trim()));
			key = '';
			await loadModels();
		} catch (err) {
			check = { state: 'error', message: toApiError(err).message };
		}
	}

	async function test() {
		if (!active) return;
		check = { state: 'busy', message: 'Testing...' };
		try {
			check = { state: 'ok', message: await api.assistant.test(active.id) };
		} catch (err) {
			check = { state: 'error', message: toApiError(err).message };
		}
	}

	function openChat() {
		ai.dialogOpen = false;
		if (!assistant.open) assistant.toggle();
	}
</script>

{#if settings}
	<div class="grid gap-4 text-sm">
		<ProviderTiles {settings} active={active?.kind} onsave={save} />

		{#if active && meta}
			<div
				class="grid grid-cols-[6rem_minmax(0,1fr)] items-center gap-x-3 gap-y-2.5 rounded-lg border p-3"
			>
				{#if meta.kind === 'openAiCompatible'}
					<label for="ai-url">Base URL</label>
					<Input
						id="ai-url"
						value={active.baseUrl}
						class="h-8 font-mono text-xs"
						onchange={(e) => update({ baseUrl: e.currentTarget.value })}
					/>
					<span></span>
					<p class="-mt-1 text-xs text-muted-foreground">
						OpenRouter, Groq, any OpenAI-compatible URL.
					</p>
				{/if}
				{#if needsKey || meta.kind === 'openAiCompatible'}
					<ApiKeyField bind:value={key} {hasKey} {needsKey} keyUrl={meta.keyUrl} onsave={saveKey} />
				{/if}

				<label for="ai-model">Model</label>
				<div class="flex gap-2">
					<Input
						id="ai-model"
						value={active.model}
						list="ai-models"
						placeholder={needsKey && !hasKey ? 'Add a key first' : 'Model name'}
						class="h-8 font-mono text-xs"
						onchange={(e) => update({ model: e.currentTarget.value })}
					/>
					<datalist id="ai-models">
						{#each models as model (model)}<option value={model}></option>{/each}
					</datalist>
					<Button size="sm" variant="outline" onclick={loadModels}>Refresh</Button>
					<Button size="sm" variant="outline" onclick={test}>Test</Button>
				</div>

				<span></span>
				<p
					class={cn(
						'flex items-center gap-1.5 text-xs',
						check.state === 'error' ? 'text-destructive' : 'text-muted-foreground'
					)}
				>
					{#if check.state === 'busy'}<LoaderIcon
							class="size-3 shrink-0 animate-spin"
						/>{:else if check.state === 'ok'}<CheckIcon
							class="size-3 shrink-0 text-success"
						/>{:else if check.state === 'error'}<CircleAlertIcon class="size-3 shrink-0" />{/if}
					{check.message ||
						(assistant.providerStatus?.local
							? `Runs on your machine: start ${meta.label} first.`
							: 'Requests go straight to the provider with your key.')}
				</p>
			</div>

			<ProviderAdvanced provider={active} onupdate={update} />
		{:else}
			<p class="text-muted-foreground">
				Pick where the chat's model runs: on this machine (Ollama, LM Studio) or in the cloud with
				your own key. The app never pays for or proxies model calls.
			</p>
		{/if}

		<div class="flex items-center justify-between border-t pt-3 text-xs text-muted-foreground">
			<span>Keys live in the OS keychain, never in files.</span>
			<Button size="sm" variant="outline" disabled={!active} onclick={openChat}>
				Open chat ({labelFor('assistant')})
			</Button>
		</div>
	</div>
{/if}
