<script lang="ts">
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { cn } from '#lib/utils.js';
	import { renderMarkdown } from '#lib/utils/markdown.js';
	import { toast } from 'svelte-sonner';
	import { api, toApiError } from '#lib/api/index.js';

	interface Props {
		/** The request's docs, read by core. */
		docs: string;
		comments: string[];
		oncomments: (comments: string[]) => void;
	}

	let { docs, comments, oncomments }: Props = $props();

	let editing = $state(false);
	/** What the user typed, shown until they leave the field and core has read it back. */
	let typed = $state<string>();
	let latest = 0;
	const text = $derived(typed ?? docs);

	/** Core writes the comment lines, so the format lives in one place; the newest edit wins. */
	async function write(value: string) {
		typed = value;
		const ticket = ++latest;
		try {
			const next = await api.requests.docs.update(comments, value);
			if (ticket === latest) oncomments(next);
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
	<div class="flex items-center justify-between">
		<p class="text-xs text-muted-foreground">
			Markdown, saved as <code>#</code> comment lines above the request in the <code>.http</code> file.
		</p>
		<div class="inline-flex rounded-md border p-0.5 text-xs">
			{#each [false, true] as mode (mode)}
				<button
					type="button"
					class={cn(
						'rounded px-2.5 py-0.5',
						editing === mode ? 'bg-muted font-medium' : 'text-muted-foreground'
					)}
					onclick={() => (editing = mode)}
				>
					{mode ? 'Edit' : 'Preview'}
				</button>
			{/each}
		</div>
	</div>

	{#if editing || !text}
		<Textarea
			class="min-h-0 flex-1 resize-none font-mono text-xs"
			value={text}
			placeholder={'What this request does, what it needs, what it returns.\n\nSupports **bold**, `code`, lists and [links](https://example.com).'}
			oninput={(e) => void write(e.currentTarget.value)}
			onfocus={() => (editing = true)}
			onblur={() => (typed = undefined)}
		/>
	{:else}
		<!-- renderMarkdown escapes all HTML in the docs before adding its own tags. -->
		<div
			class="min-h-0 flex-1 overflow-y-auto rounded-md border p-3 text-sm leading-relaxed select-text [&_a]:text-primary [&_a]:underline [&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:font-mono [&_code]:text-xs [&_h1]:mb-2 [&_h1]:text-lg [&_h1]:font-semibold [&_h2]:mb-2 [&_h2]:font-semibold [&_h3]:mb-1 [&_h3]:font-medium [&_ol]:mb-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:mb-2 [&_pre]:mb-2 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:bg-muted [&_pre]:p-2 [&_ul]:mb-2 [&_ul]:list-disc [&_ul]:pl-5"
			role="button"
			tabindex="0"
			title="Double-click to edit"
			ondblclick={() => (editing = true)}
		>
			<!-- eslint-disable-next-line svelte/no-at-html-tags -->
			{@html renderMarkdown(text)}
		</div>
	{/if}
</div>
