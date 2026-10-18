<script lang="ts">
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { settings } from '#lib/state/settings.svelte.js';
	import type { Density, Theme } from '#lib/utils/settings.js';
	import Choices from './Choices.svelte';
	import SettingRow from './SettingRow.svelte';

	const THEMES: { value: Theme; label: string }[] = [
		{ value: 'system', label: 'System' },
		{ value: 'light', label: 'Light' },
		{ value: 'dark', label: 'Dark' }
	];
	const DENSITIES: { value: Density; label: string }[] = [
		{ value: 'comfortable', label: 'Comfortable' },
		{ value: 'compact', label: 'Compact' }
	];
</script>

<SettingRow label="Theme">
	<Choices
		label="Theme"
		options={THEMES}
		value={settings.values.theme}
		onpick={(theme) => settings.update({ theme })}
	/>
</SettingRow>
<SettingRow label="Density" hint="Compact fits more on small screens.">
	<Choices
		label="Density"
		options={DENSITIES}
		value={settings.values.density}
		onpick={(density) => settings.update({ density })}
	/>
</SettingRow>
<SettingRow label="Show tips" hint="Tips in the empty editor area.">
	<Switch
		checked={settings.values.showTips}
		onCheckedChange={(showTips) => settings.update({ showTips })}
	/>
</SettingRow>
