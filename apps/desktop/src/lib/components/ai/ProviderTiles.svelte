<script lang="ts">
	import { api, type AssistantSettings, type ProviderKind } from '#lib/api/index.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
	import { confirm } from '#lib/state/confirm.svelte.js';
	import { cn } from '#lib/utils.js';
	import { PROVIDERS, providerMeta } from '#lib/utils/providers.js';
	import ProviderLogo from './ProviderLogo.svelte';

	interface Props {
		settings: AssistantSettings;
		active: ProviderKind | undefined;
		onsave: (change: Partial<AssistantSettings>) => Promise<void>;
	}

	let { settings, active, onsave }: Props = $props();

	async function pick(kind: ProviderKind) {
		const choice = providerMeta(kind);
		const existing = settings.providers.find((p) => p.kind === kind);
		const { provider, local } = existing
			? { provider: existing, local: isLocal(existing.id) }
			: await api.assistant.provider.new(kind, choice?.label ?? 'Custom');
		const cloud = !local;
		const accepted = settings.acknowledged.includes(provider.id);
		if (cloud && !accepted && !(await confirmCloud(choice?.label ?? provider.name))) return;
		await onsave({
			providers: existing ? settings.providers : [...settings.providers, provider],
			activeProvider: provider.id,
			acknowledged:
				cloud && !accepted ? [...settings.acknowledged, provider.id] : settings.acknowledged
		});
	}

	function isLocal(providerId: string): boolean {
		return assistant.status?.providers.find((p) => p.id === providerId)?.local ?? false;
	}

	/** Asked once per cloud provider: what leaves this machine when you chat. */
	function confirmCloud(label: string): Promise<boolean> {
		const bodies = settings.shareBodies
			? 'response bodies'
			: 'no response bodies (turned off in Permissions)';
		return confirm({
			title: `What gets sent to ${label}`,
			description: `Your questions, the chat's answers and what its tools return: request names, URLs, headers and ${bodies}. Secret values never: they appear as {{name}}. Changes and risky sends still need your approval.`,
			confirmLabel: `Use ${label}`
		});
	}
</script>

<div class="grid grid-cols-6 gap-2">
	{#each PROVIDERS as p (p.kind)}
		<button
			type="button"
			class={cn(
				'flex flex-col items-center gap-1.5 rounded-xl border px-1 py-3 transition',
				active === p.kind ? 'border-primary bg-primary/10 shadow-sm' : 'hover:bg-muted/60'
			)}
			aria-pressed={active === p.kind}
			onclick={() => pick(p.kind)}
		>
			<ProviderLogo mark={p.mark} gradient={p.gradient} class="size-9" />
			<span class="text-xs font-medium">{p.label}</span>
			<span class="text-[10px] text-muted-foreground">{p.where}</span>
		</button>
	{/each}
</div>
