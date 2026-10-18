<script lang="ts">
	import { cn } from '#lib/utils.js';
	import { METHODS, methodColor } from '#lib/utils/http.js';

	interface Props {
		value: string;
		onchange: (method: string) => void;
	}

	let { value, onchange }: Props = $props();
</script>

<!-- A free-text method with suggestions, so PURGE, PROPFIND and friends work too. The list arrow
	sits inside the field (14 to 16px), so the width is the text plus room for it. -->
<input
	class={cn(
		'box-content border-r bg-muted/40 px-3 font-mono text-xs font-bold uppercase outline-none',
		methodColor(value)
	)}
	style={`width: calc(${Math.max(value.length, 3)}ch + 1.25rem)`}
	{value}
	list="http-methods"
	aria-label="Method"
	spellcheck="false"
	autocomplete="off"
	oninput={(e) => onchange(e.currentTarget.value.toUpperCase().replace(/[^A-Z_-]/g, ''))}
	onfocus={(e) => e.currentTarget.select()}
/>
<datalist id="http-methods">
	{#each [...METHODS, 'TRACE', 'PURGE'] as method (method)}<option value={method}></option>{/each}
</datalist>
