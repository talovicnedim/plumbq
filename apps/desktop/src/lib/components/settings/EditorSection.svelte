<script lang="ts">
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { settings } from '#lib/state/settings.svelte.js';
	import { FONT_SIZES, TAB_SIZES } from '#lib/utils/settings.js';
	import Choices from './Choices.svelte';
	import SettingRow from './SettingRow.svelte';
</script>

<SettingRow label="Font size">
	<Choices
		label="Editor font size"
		options={FONT_SIZES.map((size) => ({ value: size, label: String(size) }))}
		value={settings.values.editorFontSize}
		onpick={(editorFontSize) => settings.update({ editorFontSize })}
	/>
</SettingRow>
<SettingRow label="Tab size">
	<Choices
		label="Tab size"
		options={TAB_SIZES.map((size) => ({ value: size, label: `${size} spaces` }))}
		value={settings.values.tabSize}
		onpick={(tabSize) => settings.update({ tabSize })}
	/>
</SettingRow>
<SettingRow label="Wrap long lines" hint="In request bodies and responses.">
	<Switch
		checked={settings.values.wordWrap}
		onCheckedChange={(wordWrap) => settings.update({ wordWrap })}
	/>
</SettingRow>
<SettingRow label="Format pasted JSON" hint="Pretty-print JSON pasted into an empty body.">
	<Switch
		checked={settings.values.formatJsonOnPaste}
		onCheckedChange={(formatJsonOnPaste) => settings.update({ formatJsonOnPaste })}
	/>
</SettingRow>
<SettingRow label="Save automatically" hint="Save edits a second after you stop typing.">
	<Switch
		checked={settings.values.autoSave}
		onCheckedChange={(autoSave) => settings.update({ autoSave })}
	/>
</SettingRow>
