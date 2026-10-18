<script lang="ts" generics="T extends string">
	import { cn } from '#lib/utils.js';

	interface Props {
		options: { value: T; label: string }[];
		value: T;
		onchange?: (value: T) => void;
	}

	let { options, value = $bindable(), onchange }: Props = $props();
</script>

<div class="grid grid-flow-col gap-1 rounded-lg bg-muted p-1">
	{#each options as option (option.value)}
		<button
			type="button"
			class={cn(
				'rounded-md py-1.5 text-sm font-medium text-muted-foreground transition-colors',
				value === option.value && 'bg-background text-foreground shadow-sm'
			)}
			onclick={() => {
				value = option.value;
				onchange?.(option.value);
			}}
		>
			{option.label}
		</button>
	{/each}
</div>
