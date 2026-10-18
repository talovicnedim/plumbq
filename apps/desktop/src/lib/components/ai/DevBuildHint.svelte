<script lang="ts">
	import { ai } from '#lib/state/ai.svelte.js';
</script>

{#if ai.binary && !ai.binary.exists}
	<p class="text-xs text-muted-foreground">
		In development, build it first: <code>cargo build -p plumbq-mcp</code>.
	</p>
{/if}
