<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import type { Header, VariableHint } from '#lib/api/index.js';
	import { fromBulk, toBulk } from '#lib/utils/bulk.js';
	import VariableInput from './VariableInput.svelte';

	interface Props {
		rows: Header[];
		/** Called with the new rows on every edit. */
		onchange: (rows: Header[]) => void;
		namePlaceholder?: string;
		valuePlaceholder?: string;
		/** Offered while typing a name, e.g. common header names. */
		suggestions?: string[];
		/** Enables "Bulk edit" as text, one `name: value` (`:`) or `name=value` (`=`) per line. */
		bulk?: ':' | '=';
		/** Colors `{{variables}}` in values and shows what each one is on hover, from core. */
		variables?: VariableHint[];
	}

	let {
		rows,
		onchange,
		namePlaceholder = 'Name',
		valuePlaceholder = 'Value',
		suggestions,
		bulk,
		variables
	}: Props = $props();

	/** The text while bulk editing; kept locally so typing isn't reformatted on every key. */
	let bulkText = $state<string | null>(null);

	function toggleBulk() {
		bulkText = bulkText === null && bulk ? toBulk(rows, bulk) : null;
	}

	const listId = `kv-${Math.random().toString(36).slice(2)}`;

	// Always one empty row at the end to type a new entry into.
	const shown = $derived([...rows, { name: '', value: '' }]);

	function edit(index: number, field: 'name' | 'value', text: string) {
		const next = shown.map((row, i) => (i === index ? { ...row, [field]: text } : row));
		onchange(next.filter((row, i) => i < next.length - 1 || row.name || row.value));
	}

	function remove(index: number) {
		onchange(rows.filter((_, i) => i !== index));
	}
</script>

{#if suggestions}
	<datalist id={listId}>
		{#each suggestions as suggestion (suggestion)}<option value={suggestion}></option>{/each}
	</datalist>
{/if}

{#if bulk}
	<div class="mb-1.5 flex justify-end">
		<button
			type="button"
			class="text-xs text-muted-foreground hover:text-foreground"
			onclick={toggleBulk}
		>
			{bulkText === null ? 'Bulk edit' : 'Key-value edit'}
		</button>
	</div>
{/if}

{#if bulkText !== null && bulk}
	<textarea
		class="h-40 w-full rounded-md border bg-transparent p-2.5 font-mono text-xs outline-none focus:ring-1 focus:ring-ring"
		spellcheck="false"
		placeholder={bulk === ':' ? 'Accept: application/json' : 'page=2'}
		bind:value={bulkText}
		oninput={(e) => onchange(fromBulk(e.currentTarget.value, bulk))}></textarea>
{:else}
	<div
		class="grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto] overflow-hidden rounded-md border text-sm"
	>
		{#each shown as row, index (index)}
			<input
				class="border-r border-b bg-transparent px-2.5 py-1.5 font-mono text-xs outline-none focus:bg-muted/50"
				value={row.name}
				placeholder={namePlaceholder}
				list={suggestions ? listId : undefined}
				spellcheck="false"
				oninput={(e) => edit(index, 'name', e.currentTarget.value)}
			/>
			{#if variables}
				<VariableInput
					bind:value={() => row.value, (text) => edit(index, 'value', text)}
					{variables}
					placeholder={valuePlaceholder}
					class="border-b focus-within:bg-muted/50"
					textClass="px-2.5 py-1.5 text-xs"
				/>
			{:else}
				<input
					class="border-b bg-transparent px-2.5 py-1.5 font-mono text-xs outline-none focus:bg-muted/50"
					value={row.value}
					placeholder={valuePlaceholder}
					spellcheck="false"
					oninput={(e) => edit(index, 'value', e.currentTarget.value)}
				/>
			{/if}
			<button
				type="button"
				class="border-b px-2 text-muted-foreground hover:text-foreground disabled:invisible"
				aria-label="Remove row"
				disabled={index === rows.length}
				onclick={() => remove(index)}
			>
				<XIcon class="size-3.5" />
			</button>
		{/each}
	</div>
{/if}
