<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type AgentPermissions, type PermissionRule } from '#lib/api/index.js';
	import EnvBadge from '#lib/components/common/EnvBadge.svelte';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import Choices from '#lib/components/settings/Choices.svelte';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
	import { environments } from '#lib/state/environments.svelte.js';

	type RuleKey = Exclude<keyof AgentPermissions, 'allowProcessEnv'>;

	// The methods that only read data come from core.
	const safeMethods = $derived(new Intl.ListFormat('en').format(ai.status?.safeMethods ?? []));

	const ROWS: { key: RuleKey; label: string; hint: string }[] = $derived([
		{
			key: 'sendSafe',
			label: 'Send GET requests',
			hint: `${safeMethods} read data without changing it.`
		},
		{
			key: 'sendWrite',
			label: 'Send requests that change data',
			hint: 'Any other method. Production always asks you to type its name.'
		},
		{
			key: 'editFiles',
			label: 'Create and edit requests',
			hint: 'New requests, edits, moves, folders, imports, examples. You see a diff first.'
		},
		{
			key: 'changeEnvironment',
			label: 'Change environments',
			hint: 'Create environments and set plain variables. Secrets stay yours.'
		},
		{
			key: 'destructive',
			label: 'Delete',
			hint: 'Requests, empty folders, variables, cookies. Always needs the strong confirmation.'
		}
	]);

	const OPTIONS: { value: PermissionRule; label: string }[] = [
		{ value: 'allow', label: 'Allow' },
		{ value: 'ask', label: 'Ask' },
		{ value: 'deny', label: 'Deny' }
	];

	const settings = $derived(ai.status?.settings);
	const permissions = $derived(settings?.permissions);
	const readOnly = $derived(settings?.readOnly ?? false);
	const hidden = $derived(settings?.hiddenEnvironments ?? []);

	function setRules(change: Partial<AgentPermissions>) {
		if (permissions) void ai.update({ permissions: { ...permissions, ...change } });
	}

	function setVisible(name: string, visible: boolean) {
		const rest = hidden.filter((h) => h !== name);
		void ai.update({ hiddenEnvironments: visible ? rest : [...rest, name] });
	}

	async function shareBodies(on: boolean) {
		const status = assistant.status;
		if (!status) return;
		try {
			assistant.setStatus(await api.assistant.update({ ...status.settings, shareBodies: on }));
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}
</script>

{#if settings && permissions}
	<div class="grid gap-4 text-sm">
		<label class="flex items-center justify-between gap-6 rounded-xl border p-4">
			<span class="grid gap-0.5">
				<span class="font-medium">Read-only</span>
				<span class="text-muted-foreground">
					The AI can look at everything, but not change anything.
				</span>
			</span>
			<Switch checked={readOnly} onCheckedChange={(on) => ai.update({ readOnly: on })} />
		</label>

		<ul class="grid list-inside list-disc gap-1 text-muted-foreground">
			<li>Changes pop up for approval; on production you type the environment name.</li>
			<li>Unticked environments are invisible to the AI.</li>
			<li>Passwords, keys and secret values never reach the AI.</li>
			<li>Everything the AI does is listed in Activity.</li>
		</ul>

		<div>
			<p class="mb-2 font-medium">Environments the AI can use</p>
			<div class="rounded-lg border">
				<QueryView
					query={environments.list}
					empty="This workspace has no environments yet."
					isEmpty={(list) => list.length === 0}
				>
					{#snippet children(list)}
						{#each list as env (env.name)}
							<label class="flex items-center gap-3 border-b px-3 py-2 last:border-b-0">
								<input
									type="checkbox"
									class="size-4 accent-foreground"
									checked={!hidden.includes(env.name)}
									onchange={(e) => setVisible(env.name, e.currentTarget.checked)}
								/>
								<span class="flex-1">{env.name}</span>
								<EnvBadge tag={env.tag} />
							</label>
						{/each}
					{/snippet}
				</QueryView>
			</div>
			<p class="mt-1.5 text-xs text-muted-foreground">
				Applies to environments with the same name in every workspace.
			</p>
		</div>

		<div class="grid gap-1 border-t pt-4">
			<p class="font-medium">What AI can see</p>
			<label class="flex items-center justify-between gap-4 border-b py-3">
				<span class="grid gap-0.5">
					<span>Share response bodies with the AI</span>
					<span class="text-xs text-muted-foreground">
						When off, the chat sees status, headers and sizes, but not what responses contain.
					</span>
				</span>
				<Switch
					checked={assistant.status?.settings.shareBodies ?? true}
					onCheckedChange={shareBodies}
				/>
			</label>
			{#if readOnly}
				<p class="py-3 text-xs text-muted-foreground">
					Read-only is on: only reading and {safeMethods} sends are allowed.
				</p>
			{:else}
				{#each ROWS as row (row.key)}
					<div class="flex items-center justify-between gap-4 border-b py-3">
						<span class="grid gap-0.5">
							<span>{row.label}</span>
							<span class="text-xs text-muted-foreground">{row.hint}</span>
						</span>
						<Choices
							label={row.label}
							options={OPTIONS}
							value={permissions[row.key]}
							onpick={(value) => setRules({ [row.key]: value })}
						/>
					</div>
				{/each}
			{/if}
			<label class="flex items-center justify-between gap-4 py-3">
				<span class="grid gap-0.5">
					<span>Read OS environment variables</span>
					<span class="text-xs text-muted-foreground">
						Lets requests use <code>{'{{$processEnv NAME}}'}</code> when the AI sends them.
					</span>
				</span>
				<Switch
					checked={permissions.allowProcessEnv}
					onCheckedChange={(allowProcessEnv) => setRules({ allowProcessEnv })}
				/>
			</label>
			<p class="text-xs text-muted-foreground">
				Even when allowed, you're asked when a request would send secrets to a host that isn't in
				the environment, or a URL would point somewhere new.
			</p>
		</div>
	</div>
{/if}
