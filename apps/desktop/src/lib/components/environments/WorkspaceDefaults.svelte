<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { toApiError } from '#lib/api/index.js';
	import KeyValueEditor from '#lib/components/common/KeyValueEditor.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { EnvironmentEditor } from '#lib/state/environment-editor.svelte.js';
	import { environments } from '#lib/state/environments.svelte.js';

	interface Props {
		editor: EnvironmentEditor;
	}

	let { editor }: Props = $props();

	let saving = $state(false);

	async function save() {
		saving = true;
		try {
			await environments.saveDefaults(
				Object.fromEntries(editor.defaults.filter((r) => r.name).map((r) => [r.name, r.value]))
			);
			toast.success('Workspace defaults saved');
		} catch (err) {
			toast.error(toApiError(err).message);
		} finally {
			saving = false;
		}
	}
</script>

<div class="grid min-h-0 content-start gap-4 overflow-y-auto">
	<p class="text-sm text-muted-foreground">
		Variables every environment starts with, saved in the workspace's root
		<code>folder.toml</code>. An environment's own value wins. Good for things that are the same
		everywhere, like <code>apiVersion</code>. Never put secrets here: they would be committed.
	</p>
	<KeyValueEditor
		rows={editor.defaults}
		onchange={(next) => (editor.defaults = next)}
		namePlaceholder="apiVersion"
		bulk="="
	/>
	<div class="flex items-center justify-end gap-3">
		{#if editor.defaultsChangedOnDisk}
			<span class="text-xs text-warning">Changed on disk since you started editing</span>
		{/if}
		<Button onclick={save} disabled={saving}>Save</Button>
	</div>
</div>
