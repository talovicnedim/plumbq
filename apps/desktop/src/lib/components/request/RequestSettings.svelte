<script lang="ts">
	import type { FileVariable, Header, RequestSettings } from '#lib/api/index.js';
	import KeyValueEditor from '#lib/components/common/KeyValueEditor.svelte';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { workspaceSettings } from '#lib/state/workspace-settings.svelte.js';

	interface Props {
		settings: RequestSettings;
		variables: FileVariable[];
		onsettings: (settings: RequestSettings) => void;
		onvariables: (variables: FileVariable[]) => void;
	}

	let { settings, variables, onsettings, onvariables }: Props = $props();

	const toggles: { key: 'noRedirect' | 'noCookieJar' | 'insecure'; label: string; hint: string }[] =
		[
			{ key: 'noRedirect', label: "Don't follow redirects", hint: '# @no-redirect' },
			{ key: 'noCookieJar', label: "Don't use the cookie jar", hint: '# @no-cookie-jar' },
			{ key: 'insecure', label: 'Accept invalid TLS certificates', hint: '# @insecure' }
		];

	function seconds(text: string): number | null {
		const value = Number.parseInt(text, 10);
		return Number.isFinite(value) && value > 0 ? value : null;
	}
</script>

<div class="grid max-w-2xl gap-5 text-sm">
	<div class="grid gap-2">
		{#each toggles as toggle (toggle.key)}
			<label class="flex items-center justify-between gap-4">
				<span>
					{toggle.label}
					<code class="ml-1 text-xs text-muted-foreground">{toggle.hint}</code>
				</span>
				<Switch
					checked={settings[toggle.key]}
					onCheckedChange={(checked) => onsettings({ ...settings, [toggle.key]: checked })}
				/>
			</label>
		{/each}
	</div>

	<div class="grid grid-cols-2 gap-3">
		<label class="grid gap-1.5">
			Timeout (seconds)
			<input
				class="h-8 rounded-md border bg-transparent px-2 text-xs"
				type="number"
				min="1"
				placeholder="workspace default"
				value={settings.timeoutSecs ?? ''}
				oninput={(e) => onsettings({ ...settings, timeoutSecs: seconds(e.currentTarget.value) })}
			/>
		</label>
		<label class="grid gap-1.5">
			Connection timeout (seconds)
			<input
				class="h-8 rounded-md border bg-transparent px-2 text-xs"
				type="number"
				min="1"
				placeholder="workspace default"
				value={settings.connectionTimeoutSecs ?? ''}
				oninput={(e) =>
					onsettings({ ...settings, connectionTimeoutSecs: seconds(e.currentTarget.value) })}
			/>
		</label>
	</div>

	<div class="grid gap-1.5">
		File variables
		<KeyValueEditor
			rows={variables.map((v): Header => ({ name: v.name, value: v.value }))}
			onchange={(rows) => onvariables(rows.filter((r) => r.name))}
			namePlaceholder="name"
			valuePlaceholder={'value, may use {{variables}}'}
		/>
		<p class="text-xs text-muted-foreground">
			Saved as <code>@name = value</code> lines (REST Client syntax) and usable as
			<code>{'{{name}}'}</code> in every request of this file. They win over environment variables.
		</p>
	</div>

	<p class="text-xs text-muted-foreground">
		Settings are saved as JetBrains HTTP Client directives in the .http file. Defaults for every
		request (proxy, certificates, timeouts) are in
		<button
			type="button"
			class="text-primary hover:underline"
			onclick={() => (workspaceSettings.open = true)}
		>
			workspace settings
		</button>.
	</p>
</div>
