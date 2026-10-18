<script lang="ts">
	import type { ProviderConfig } from '#lib/api/index.js';
	import { Input } from '#lib/components/ui/input/index.js';

	interface Props {
		provider: ProviderConfig;
		onupdate: (change: Partial<ProviderConfig>) => void;
	}

	let { provider, onupdate }: Props = $props();
</script>

<details class="text-xs">
	<summary class="cursor-pointer text-muted-foreground">Advanced</summary>
	<div class="mt-2 grid grid-cols-[6rem_minmax(0,1fr)] items-center gap-x-3 gap-y-2 pl-3">
		{#if provider.kind !== 'openAiCompatible'}
			<label for="ai-base-url">Base URL</label>
			<Input
				id="ai-base-url"
				value={provider.baseUrl}
				class="h-8 font-mono text-xs"
				onchange={(e) => onupdate({ baseUrl: e.currentTarget.value })}
			/>
		{/if}
		<label for="ai-temperature">Temperature</label>
		<Input
			id="ai-temperature"
			type="number"
			step="0.1"
			min="0"
			max="2"
			class="h-8 w-24"
			value={provider.temperature ?? ''}
			placeholder="default"
			onchange={(e) => {
				const v = e.currentTarget.value;
				onupdate({ temperature: v === '' ? null : Number(v) });
			}}
		/>
	</div>
</details>
