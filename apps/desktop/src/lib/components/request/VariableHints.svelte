<script lang="ts">
	import type { VariableHint } from '#lib/api/index.js';
	import { missingLine } from '#lib/utils/variables.js';

	interface Props {
		/** Every `{{variable}}` of the request and how it resolves, from core. */
		variables: VariableHint[];
		/** Why core couldn't look at the draft. */
		error: string | undefined;
		/** The selected environment's name. */
		environment: string | null;
		onmanage: () => void;
	}

	let { variables, error, environment, onmanage }: Props = $props();

	const missing = $derived(missingLine(variables, environment));
</script>

{#if error}
	<p class="text-xs text-destructive">{error}</p>
{/if}
{#if missing}
	<p class="-mt-1 flex items-center gap-1.5 text-xs text-muted-foreground">
		<span class="size-1.5 shrink-0 rounded-full bg-destructive"></span>
		<span class="min-w-0 truncate">{missing.text}</span>
		<button type="button" class="shrink-0 text-primary hover:underline" onclick={onmanage}>
			{missing.action}
		</button>
	</p>
{/if}
