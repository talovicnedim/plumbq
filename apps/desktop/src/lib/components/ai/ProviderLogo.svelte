<script lang="ts">
	import { cn } from '#lib/utils.js';

	interface Props {
		mark: string;
		/** Gradient stops from the provider color tokens, e.g. `from-provider-anthropic to-provider-anthropic-deep`. */
		gradient: string;
		class?: string;
	}

	let { mark, gradient, class: className }: Props = $props();
</script>

<span
	class={cn(
		'inline-flex size-8 shrink-0 items-center justify-center rounded-full bg-linear-to-br text-xs font-semibold text-white shadow-sm',
		gradient,
		className
	)}
	aria-hidden="true"
>
	{mark}
</span>
