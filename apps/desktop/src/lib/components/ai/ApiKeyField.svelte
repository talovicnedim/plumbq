<script lang="ts">
	import { api } from '#lib/api/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';

	interface Props {
		value: string;
		hasKey: boolean;
		needsKey: boolean;
		keyUrl: string | undefined;
		onsave: () => void;
	}

	let { value = $bindable(), hasKey, needsKey, keyUrl, onsave }: Props = $props();
</script>

<label for="ai-key">API key</label>
<div class="flex gap-2">
	<Input
		id="ai-key"
		type="password"
		autocomplete="off"
		class="h-8"
		bind:value
		placeholder={hasKey
			? '•••••••• Saved in your keychain'
			: needsKey
				? 'Saved in your keychain'
				: 'Optional, saved in your keychain'}
		onblur={onsave}
		onkeydown={(e) => e.key === 'Enter' && onsave()}
	/>
	{#if keyUrl}
		<Button size="sm" variant="ghost" onclick={() => keyUrl && api.app.openUrl(keyUrl)}
			>Get a key</Button
		>
	{/if}
</div>
