<script lang="ts">
	import type { BodyFile, Header, VariableHint } from '#lib/api/index.js';
	import CodeEditor from '#lib/components/common/CodeEditor.svelte';
	import KeyValueEditor from '#lib/components/common/KeyValueEditor.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { cn } from '#lib/utils.js';
	import FileIcon from '@lucide/svelte/icons/file';
	import { chooseBodyFile } from '#lib/state/body-files.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import {
		BODY_KINDS,
		BODY_LABEL,
		bodyKind,
		boundaryOf,
		formBody,
		parseForm,
		withBodyKind,
		type BodyKind
	} from '#lib/utils/body.js';
	import MultipartEditor from './MultipartEditor.svelte';
	import { variableMarks } from './variable-marks.js';

	interface Props {
		body: string;
		headers: Header[];
		onbody: (body: string) => void;
		/** The body kind lives in the Content-Type header, so changing it edits headers. */
		onheaders: (headers: Header[]) => void;
		/** Identifies the request, so switching requests recreates the editor. */
		key: string;
		/** The file a `< ./path` body sends, from core; undefined until it answers. */
		fileReference: string | null | undefined;
		/** How each `{{variable}}` resolves, from core. */
		variables: VariableHint[];
	}

	let { body, headers, onbody, onheaders, key, fileReference, variables }: Props = $props();

	/** The file just chosen, shown at once: core's answer comes a moment later. */
	let chosen = $state<BodyFile>();

	// Core's answer lags edits, so it counts only while the body still names that file.
	const filePath = $derived(
		chosen?.body === body
			? chosen.path
			: fileReference && body.includes(fileReference)
				? fileReference
				: null
	);
	const kind = $derived(bodyKind(headers, body, filePath));
	const boundary = $derived(boundaryOf(headers));
	const marks = $derived(variableMarks(variables, environments.selected));

	function setKind(next: BodyKind) {
		onheaders(withBodyKind(headers, next));
		if (next === 'none' || next === 'multipart' || (kind === 'file' && next !== 'file')) onbody('');
		if (next === 'file') void chooseFile();
	}

	async function chooseFile() {
		const file = await chooseBodyFile(key);
		if (!file) return;
		chosen = file;
		onbody(file.body);
		onheaders([
			...headers.filter((h) => h.name.toLowerCase() !== 'content-type'),
			{ name: 'Content-Type', value: file.contentType }
		]);
	}

	function format() {
		try {
			onbody(JSON.stringify(JSON.parse(body), null, 2));
		} catch {
			// Invalid JSON stays as typed; the editor already highlights the error.
		}
	}
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
	<div class="flex items-center justify-between">
		<div class="inline-flex rounded-md border p-0.5">
			{#each BODY_KINDS as value (value)}
				<button
					type="button"
					class={cn(
						'rounded px-2.5 py-1 text-xs',
						kind === value ? 'bg-muted font-medium' : 'text-muted-foreground hover:text-foreground'
					)}
					onclick={() => setKind(value)}
				>
					{BODY_LABEL[value]}
				</button>
			{/each}
		</div>
		{#if kind === 'json'}
			<Button size="sm" variant="ghost" class="h-7 text-xs" onclick={format}>Format</Button>
		{/if}
	</div>

	{#if kind === 'none'}
		<p class="py-8 text-center text-sm text-muted-foreground">This request has no body.</p>
	{:else if kind === 'file'}
		<div class="flex flex-col items-center gap-3 rounded-md border border-dashed py-8 text-sm">
			{#if filePath}
				<p class="flex items-center gap-2 font-mono text-xs">
					<FileIcon class="size-4 text-primary" />
					{filePath}
				</p>
			{/if}
			<Button size="sm" variant="outline" onclick={chooseFile}>
				{filePath ? 'Choose another file…' : 'Choose a file…'}
			</Button>
			<p class="text-xs text-muted-foreground">
				Sent as is, read when you send. Stored as <code>&lt; ./path</code>, relative to the .http
				file.
			</p>
		</div>
	{:else if kind === 'multipart' && boundary}
		<div class="min-h-0 overflow-y-auto">
			<MultipartEditor {body} {boundary} {onbody} requestId={key} />
		</div>
	{:else if kind === 'form'}
		<div class="min-h-0 overflow-y-auto">
			<KeyValueEditor
				rows={parseForm(body)}
				onchange={(rows) => onbody(formBody(rows))}
				namePlaceholder="field"
				{variables}
			/>
		</div>
	{:else}
		{#key `${key}:${kind}`}
			<CodeEditor
				bind:value={() => body, onbody}
				json={kind === 'json'}
				extensions={marks}
				class="min-h-0 flex-1"
			/>
		{/key}
	{/if}
</div>
