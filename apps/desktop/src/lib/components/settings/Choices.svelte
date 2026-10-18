<script lang="ts" generics="T extends string | number">
	import { cn } from '#lib/utils.js';

	interface Props {
		options: { value: T; label: string }[];
		value: T;
		onpick: (value: T) => void;
		label: string;
	}

	let { options, value, onpick, label }: Props = $props();
</script>

<div role="radiogroup" aria-label={label} class="inline-flex rounded-md bg-muted p-0.5 text-sm">
	{#each options as option (option.value)}
		<button
			type="button"
			role="radio"
			aria-checked={value === option.value}
			class={cn(
				'rounded px-3 py-1 text-muted-foreground',
				value === option.value && 'bg-background text-foreground shadow-sm'
			)}
			onclick={() => onpick(option.value)}
		>
			{option.label}
		</button>
	{/each}
</div>
