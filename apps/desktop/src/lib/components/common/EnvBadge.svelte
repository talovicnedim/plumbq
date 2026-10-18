<script lang="ts">
	import type { EnvironmentTag } from '#lib/api/index.js';
	import { cn } from '#lib/utils.js';
	import { ENVIRONMENT_BG, ENVIRONMENT_SHORT } from '#lib/utils/environments.js';

	interface Props {
		tag: EnvironmentTag;
		class?: string;
	}

	let { tag, class: className }: Props = $props();
</script>

<span
	class={cn(
		'inline-flex h-5 items-center rounded px-1.5 text-[10px] font-semibold tracking-wider text-white',
		ENVIRONMENT_BG[tag],
		className
	)}
>
	{ENVIRONMENT_SHORT[tag]}
</span>
