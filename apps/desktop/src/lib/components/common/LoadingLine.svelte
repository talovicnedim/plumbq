<script lang="ts">
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import { cn } from '#lib/utils.js';

	interface Props {
		label?: string;
		class?: string;
	}

	let { label = 'Loading…', class: className }: Props = $props();
</script>

<div
	role="status"
	class={cn('flex items-center justify-center gap-2 py-6 text-sm text-muted-foreground', className)}
>
	<LoaderIcon class="size-4 animate-spin" />
	{label}
</div>
