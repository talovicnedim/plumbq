<script lang="ts">
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import { toast } from 'svelte-sonner';
	import { api, toApiError } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { settings } from '#lib/state/settings.svelte.js';
	import { checkForUpdates } from '#lib/state/updates.js';
	import SettingRow from './SettingRow.svelte';

	let version = $state('');
	$effect(() => {
		api.app
			.version()
			.then((v) => (version = v))
			.catch((err) => toast.error(toApiError(err).message));
	});

	async function showLog() {
		try {
			await api.app.revealLog();
		} catch (err) {
			toast.error(`Couldn't open the log folder: ${toApiError(err).message}`);
		}
	}
</script>

<SettingRow label="Plumbq" hint={version ? `Version ${version}` : undefined}>
	<Button size="sm" variant="outline" onclick={() => checkForUpdates({ manual: true })}>
		Check for updates
	</Button>
</SettingRow>
<SettingRow label="Check for updates on launch">
	<Switch
		checked={settings.values.checkForUpdates}
		onCheckedChange={(checkForUpdates) => settings.update({ checkForUpdates })}
	/>
</SettingRow>
<SettingRow label="Log file" hint="Never contains request or response contents.">
	<Button size="sm" variant="outline" onclick={showLog}>
		<FileTextIcon /> Show log file
	</Button>
</SettingRow>
