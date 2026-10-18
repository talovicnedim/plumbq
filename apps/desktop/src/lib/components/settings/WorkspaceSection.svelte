<script lang="ts">
	import { goto } from '$app/navigation';
	import { Button } from '#lib/components/ui/button/index.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { workspaceSettings } from '#lib/state/workspace-settings.svelte.js';
	import SettingRow from './SettingRow.svelte';

	async function openWorkspaceSettings() {
		await goto('/');
		workspaceSettings.open = true;
	}
</script>

<SettingRow
	label="Workspace settings"
	hint={workspace.root
		? 'Proxy, certificates, timeouts and redirects for this workspace (stored in folder.toml).'
		: 'Open a workspace first.'}
>
	<Button size="sm" variant="outline" disabled={!workspace.root} onclick={openWorkspaceSettings}>
		Open…
	</Button>
</SettingRow>
