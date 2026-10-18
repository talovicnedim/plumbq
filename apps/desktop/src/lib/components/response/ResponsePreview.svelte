<script lang="ts">
	import type { Response } from '#lib/api/index.js';
	import { bodyBlob, type PreviewKind } from '#lib/utils/preview.js';

	interface Props {
		response: Response;
		kind: PreviewKind;
		contentType: string;
	}

	let { response, kind, contentType }: Props = $props();

	let url = $state<string>();

	// A blob URL per response; revoked when it changes so previews don't leak memory.
	$effect(() => {
		if (kind === 'html') return;
		const created = URL.createObjectURL(bodyBlob(response, contentType));
		url = created;
		return () => URL.revokeObjectURL(created);
	});
</script>

<div
	class="flex h-full min-h-0 items-center justify-center overflow-auto rounded-md border bg-muted/20"
>
	{#if kind === 'html'}
		<!-- No scripts, forms or same-origin access: just how the page looks. -->
		<iframe title="HTML preview" sandbox="" srcdoc={response.body} class="h-full w-full bg-white"
		></iframe>
	{:else if !url}
		<span class="text-sm text-muted-foreground">Loading preview…</span>
	{:else if kind === 'image'}
		<!-- SVG as an <img> never runs its scripts. -->
		<img src={url} alt="Response" class="max-h-full max-w-full object-contain" />
	{:else if kind === 'pdf'}
		<iframe title="PDF preview" src={url} class="h-full w-full"></iframe>
	{:else if kind === 'audio'}
		<audio controls src={url}></audio>
	{:else if kind === 'video'}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video controls src={url} class="max-h-full max-w-full"></video>
	{/if}
</div>
