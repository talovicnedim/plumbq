<script lang="ts">
	import { guideOffsets } from '#lib/utils/tree.js';

	interface Props {
		depth: number;
	}

	let { depth }: Props = $props();
</script>

{#each guideOffsets(depth) as left (left)}
	<span
		class="pointer-events-none absolute top-0 -bottom-px w-px bg-sidebar-border"
		style:left={`${left}rem`}
	></span>
{/each}
