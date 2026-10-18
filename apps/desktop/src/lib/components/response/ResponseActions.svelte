<script lang="ts">
	import BookmarkPlusIcon from '@lucide/svelte/icons/bookmark-plus';
	import CheckIcon from '@lucide/svelte/icons/check';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import GitCompareIcon from '@lucide/svelte/icons/git-compare';
	import WrapTextIcon from '@lucide/svelte/icons/wrap-text';
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type Response } from '#lib/api/index.js';
	import { bodyModes } from '#lib/state/body-modes.svelte.js';
	import { prompt } from '#lib/state/prompt.svelte.js';
	import { cn } from '#lib/utils.js';
	import { prettyBody } from '#lib/utils/http.js';
	import { suggestFilename, type BodyMode } from '#lib/utils/preview.js';

	interface Props {
		requestId: string;
		response: Response;
		contentType: string | undefined;
		mode: BodyMode;
		modes: BodyMode[];
		wrap: boolean;
		oncompare: () => void;
	}

	let {
		requestId,
		response,
		contentType,
		mode,
		modes,
		wrap = $bindable(),
		oncompare
	}: Props = $props();

	let copied = $state(false);

	async function createExample() {
		const name = await prompt({
			title: 'Save as example',
			label: 'Name',
			confirmLabel: 'Save',
			placeholder: `${response.status} ${response.statusText}`.trim(),
			initial: `${response.status} ${response.statusText}`.trim()
		});
		if (!name) return;
		try {
			await api.examples.create(requestId, name, response);
			toast.success(`Saved example "${name}"`, {
				description: 'Stored next to the request in a .examples.json file.'
			});
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async function exportBody() {
		try {
			const name = suggestFilename(response.headers, response.sent.url);
			const path = await api.responses.export(response, name);
			if (path) toast.success('Response saved', { description: path });
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async function copy() {
		await navigator.clipboard.writeText(prettyBody(response.body, contentType));
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}
</script>

<div class="flex items-center gap-1">
	<div class="inline-flex rounded-md border p-0.5 text-xs">
		{#each modes as option (option)}
			<button
				type="button"
				class={cn(
					'rounded px-2 py-0.5 capitalize',
					mode === option ? 'bg-muted font-medium' : 'text-muted-foreground'
				)}
				onclick={() => bodyModes.set(contentType, option)}
			>
				{option}
			</button>
		{/each}
	</div>
	<button
		type="button"
		class={cn(
			'rounded-md p-1.5 hover:bg-muted hover:text-foreground',
			wrap ? 'text-foreground' : 'text-muted-foreground'
		)}
		title={wrap ? "Don't wrap long lines" : 'Wrap long lines'}
		aria-label="Wrap long lines"
		aria-pressed={wrap}
		onclick={() => (wrap = !wrap)}
	>
		<WrapTextIcon class="size-3.5" />
	</button>
	<button
		type="button"
		class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
		title="Save as example (committed next to the request, readable by AI agents)"
		aria-label="Save as example"
		onclick={createExample}
	>
		<BookmarkPlusIcon class="size-3.5" />
	</button>
	<button
		type="button"
		class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
		title="Compare with an example or an earlier response"
		onclick={oncompare}
	>
		<GitCompareIcon class="size-3.5" />
	</button>
	<button
		type="button"
		class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
		title="Save body to a file"
		aria-label="Save body to a file"
		onclick={exportBody}
	>
		<DownloadIcon class="size-3.5" />
	</button>
	<button
		type="button"
		class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-40"
		title="Copy body"
		aria-label="Copy body"
		disabled={response.binary}
		onclick={copy}
	>
		{#if copied}<CheckIcon class="size-3.5" />{:else}<CopyIcon class="size-3.5" />{/if}
	</button>
</div>
