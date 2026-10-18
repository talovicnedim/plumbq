<script lang="ts">
	import type { EnvironmentTag } from '#lib/api/index.js';
	import { ENVIRONMENT_LABEL, ENVIRONMENT_TAGS } from '#lib/utils/environments.js';

	interface Props {
		tag: EnvironmentTag;
	}

	let { tag = $bindable() }: Props = $props();
</script>

<label class="grid gap-1.5 text-sm">
	Type
	<select class="rounded-md border bg-transparent px-2 py-1.5 text-sm" bind:value={tag}>
		{#each ENVIRONMENT_TAGS as option (option)}
			<option value={option}>{ENVIRONMENT_LABEL[option]}</option>
		{/each}
	</select>
	{#if tag === 'production'}
		<span class="text-xs text-muted-foreground">
			Anything other than GET, HEAD and OPTIONS asks for confirmation, including from AI agents.
		</span>
	{/if}
</label>
