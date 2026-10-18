<script lang="ts">
	import FileIcon from '@lucide/svelte/icons/file';
	import XIcon from '@lucide/svelte/icons/x';
	import type { MultipartField } from '#lib/api/index.js';
	import { chooseBodyFile } from '#lib/state/body-files.js';
	import { MultipartFields } from '#lib/state/multipart.svelte.js';
	import { cn } from '#lib/utils.js';

	interface Props {
		body: string;
		boundary: string;
		onbody: (body: string) => void;
		requestId: string;
	}

	let { body, boundary, onbody, requestId }: Props = $props();

	const draft = new MultipartFields();

	$effect(() => draft.read(body, boundary));

	const rows = $derived(draft.fields ?? []);
	// Always one empty row at the end to type a new field into.
	const shown = $derived([...rows, { name: '', value: '', file: false }]);

	function onchange(next: MultipartField[]) {
		draft.write(next, boundary, onbody);
	}

	function edit(index: number, change: Partial<MultipartField>) {
		const next = shown.map((row, i) => (i === index ? { ...row, ...change } : row));
		onchange(next.filter((row, i) => i < next.length - 1 || row.name || row.value));
	}

	async function chooseFile(index: number) {
		const file = await chooseBodyFile(requestId);
		if (file) edit(index, { value: file.path, file: true, name: shown[index].name || 'file' });
	}
</script>

{#if draft.fields}
	<div
		class="grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_auto_auto] overflow-hidden rounded-md border text-sm"
	>
		{#each shown as row, index (index)}
			<input
				class="border-r border-b bg-transparent px-2.5 py-1.5 font-mono text-xs outline-none focus:bg-muted/50"
				value={row.name}
				placeholder="field"
				spellcheck="false"
				oninput={(e) => edit(index, { name: e.currentTarget.value })}
			/>
			{#if row.file}
				<button
					type="button"
					class="flex items-center gap-1.5 border-b px-2.5 py-1.5 text-left font-mono text-xs hover:bg-muted/50"
					title="Choose another file"
					onclick={() => chooseFile(index)}
				>
					<FileIcon class="size-3.5 shrink-0 text-primary" />
					<span class="truncate">{row.value}</span>
				</button>
			{:else}
				<input
					class="border-b bg-transparent px-2.5 py-1.5 font-mono text-xs outline-none focus:bg-muted/50"
					value={row.value}
					placeholder="value"
					spellcheck="false"
					oninput={(e) => edit(index, { value: e.currentTarget.value })}
				/>
			{/if}
			<button
				type="button"
				class={cn(
					'border-b border-l px-2 text-xs',
					row.file ? 'text-primary' : 'text-muted-foreground hover:text-foreground'
				)}
				title={row.file ? 'Use text instead' : 'Send a file'}
				onclick={() => (row.file ? edit(index, { file: false, value: '' }) : chooseFile(index))}
			>
				{row.file ? 'Text' : 'File…'}
			</button>
			<button
				type="button"
				class="border-b px-2 text-muted-foreground hover:text-foreground disabled:invisible"
				aria-label="Remove field"
				disabled={index === rows.length}
				onclick={() => onchange(rows.filter((_, i) => i !== index))}
			>
				<XIcon class="size-3.5" />
			</button>
		{/each}
	</div>
{/if}
