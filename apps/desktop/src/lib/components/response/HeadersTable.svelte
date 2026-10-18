<script lang="ts">
	import type { Header } from '#lib/api/index.js';

	interface Props {
		headers: Header[];
	}

	let { headers }: Props = $props();
</script>

<table class="w-full font-mono text-xs select-text">
	<tbody>
		{#each headers as header, i (i)}
			<tr class="border-b last:border-0">
				<td class="py-1.5 pr-4 align-top whitespace-nowrap text-muted-foreground">{header.name}</td>
				<td class="py-1.5 break-all">{header.value}</td>
			</tr>
		{/each}
	</tbody>
</table>
