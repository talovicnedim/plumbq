<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type HttpSettings_Deserialize } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { workspaceSettings } from '#lib/state/workspace-settings.svelte.js';

	let settings = $state<HttpSettings_Deserialize>({});
	let busy = $state(false);
	/** Typed only when changing it; the stored password is never read back. */
	let pfxPassword = $state('');
	let hasPfxPassword = $state(false);
	let defaultTimeout = $state<number>();

	$effect(() => {
		if (!workspaceSettings.open) return;
		pfxPassword = '';
		api.folders.getSettings('').then(
			(folder) => (settings = { ...(folder.settings ?? {}) }),
			(err) => toast.error(toApiError(err).message)
		);
		api.workspace.httpDefaults().then((defaults) => (defaultTimeout = defaults.timeoutSecs));
		api.workspace.hasPfxPassword().then(
			(has) => (hasPfxPassword = has),
			(err) => toast.error(toApiError(err).message)
		);
	});

	function seconds(text: string): number | null {
		const value = Number.parseInt(text, 10);
		return Number.isFinite(value) && value > 0 ? value : null;
	}

	async function clearPfxPassword() {
		try {
			// An empty password removes it from the keychain.
			await api.workspace.setPfxPassword('');
			hasPfxPassword = false;
			pfxPassword = '';
			toast.success('PFX password removed from the keychain');
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async function save() {
		busy = true;
		try {
			// Re-read first so order, headers and variables edited elsewhere aren't overwritten.
			const folder = await api.folders.getSettings('');
			await api.folders.updateSettings('', { ...folder, settings });
			if (pfxPassword) await api.workspace.setPfxPassword(pfxPassword);
			toast.success('Workspace settings saved');
			workspaceSettings.open = false;
		} catch (err) {
			toast.error(toApiError(err).message);
		} finally {
			busy = false;
		}
	}
</script>

<Dialog.Root bind:open={workspaceSettings.open}>
	<Dialog.Content class="sm:max-w-xl">
		<Dialog.Header>
			<Dialog.Title>Workspace settings</Dialog.Title>
			<Dialog.Description>
				HTTP defaults for every request, saved in the root <code>folder.toml</code>. A request's own
				Settings tab wins.
			</Dialog.Description>
		</Dialog.Header>

		<form
			class="grid gap-4 text-sm"
			onsubmit={(e) => {
				e.preventDefault();
				void save();
			}}
		>
			<div class="grid grid-cols-2 gap-3">
				<label class="grid gap-1.5">
					Timeout (seconds)
					<Input
						type="number"
						min="1"
						placeholder={defaultTimeout?.toString() ?? ''}
						value={settings.timeoutSecs ?? ''}
						oninput={(e) => (settings.timeoutSecs = seconds(e.currentTarget.value))}
					/>
				</label>
				<label class="grid gap-1.5">
					Connection timeout (seconds)
					<Input
						type="number"
						min="1"
						placeholder="none"
						value={settings.connectTimeoutSecs ?? ''}
						oninput={(e) => (settings.connectTimeoutSecs = seconds(e.currentTarget.value))}
					/>
				</label>
			</div>

			<label class="flex items-center justify-between">
				Follow redirects
				<Switch
					checked={settings.followRedirects ?? true}
					onCheckedChange={(checked) => (settings.followRedirects = checked ? null : false)}
				/>
			</label>
			<label class="flex items-center justify-between">
				<span>
					Verify TLS certificates
					<span class="block text-xs text-muted-foreground"
						>Turn off for self-signed local servers.</span
					>
				</span>
				<Switch
					checked={settings.verifyTls ?? true}
					onCheckedChange={(checked) => (settings.verifyTls = checked ? null : false)}
				/>
			</label>

			<label class="grid gap-1.5">
				Proxy
				<Input
					class="font-mono text-xs"
					placeholder="system proxy (empty), none, http://…, socks5://…"
					bind:value={() => settings.proxy ?? '', (v) => (settings.proxy = v)}
				/>
			</label>

			<div class="grid gap-1.5">
				Certificates
				<Input
					class="font-mono text-xs"
					placeholder="CA certificate (PEM), e.g. certs/ca.pem"
					bind:value={() => settings.caCert ?? '', (v) => (settings.caCert = v)}
				/>
				<div class="grid grid-cols-2 gap-2">
					<Input
						class="font-mono text-xs"
						placeholder="Client certificate (PEM)"
						bind:value={() => settings.clientCert ?? '', (v) => (settings.clientCert = v)}
					/>
					<Input
						class="font-mono text-xs"
						placeholder="Client key (PEM)"
						bind:value={() => settings.clientKey ?? '', (v) => (settings.clientKey = v)}
					/>
				</div>
				<div class="grid grid-cols-2 gap-2">
					<Input
						class="font-mono text-xs"
						placeholder="or PKCS#12 file (.pfx / .p12)"
						bind:value={() => settings.clientPfx ?? '', (v) => (settings.clientPfx = v)}
					/>
					<Input
						type="password"
						class="font-mono text-xs"
						autocomplete="off"
						placeholder={hasPfxPassword ? 'Password stored in keychain' : 'PFX password'}
						bind:value={pfxPassword}
					/>
				</div>
				{#if hasPfxPassword}
					<button
						type="button"
						class="w-fit text-xs text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
						onclick={clearPfxPassword}
					>
						Remove the stored PFX password
					</button>
				{/if}
				<p class="text-xs text-muted-foreground">
					Paths are relative to the workspace (or absolute). Keep private keys out of git. The PFX
					password goes to the OS keychain, never to <code>folder.toml</code>.
				</p>
			</div>

			<Dialog.Footer class="gap-2">
				<Button type="button" variant="ghost" onclick={() => (workspaceSettings.open = false)}>
					Cancel
				</Button>
				<Button type="submit" disabled={busy}>Save</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
