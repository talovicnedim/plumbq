<script lang="ts">
	import PlusIcon from '@lucide/svelte/icons/plus';
	import XIcon from '@lucide/svelte/icons/x';
	import { toast } from 'svelte-sonner';
	import { api, toApiError } from '#lib/api/index.js';

	interface Props {
		/** The request's tags, read by core. */
		tags: string[];
		comments: string[];
		oncomments: (comments: string[]) => void;
	}

	let { tags, comments, oncomments }: Props = $props();

	let adding = $state(false);
	let input = $state('');

	/** Core writes the `# @tag` line, so the format lives in one place. */
	async function setTags(next: string[]) {
		try {
			oncomments(await api.requests.tags.update(comments, next));
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	function add() {
		const fresh = input
			.split(/[,\s]+/)
			.map((t) => t.trim())
			.filter((t) => t && !tags.includes(t));
		if (fresh.length) void setTags([...tags, ...fresh]);
		input = '';
		adding = false;
	}
</script>

<div class="flex flex-wrap items-center gap-1.5 text-xs">
	{#each tags as tag (tag)}
		<span
			class="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-muted-foreground"
		>
			#{tag}
			<button
				type="button"
				class="hover:text-foreground"
				aria-label={`Remove tag ${tag}`}
				onclick={() => void setTags(tags.filter((t) => t !== tag))}
			>
				<XIcon class="size-3" />
			</button>
		</span>
	{/each}
	{#if adding}
		<input
			class="w-28 rounded-full border bg-transparent px-2 py-0.5 outline-none focus:ring-1 focus:ring-ring"
			placeholder="smoke, auth"
			bind:value={input}
			onkeydown={(e) => {
				if (e.key === 'Enter') add();
				if (e.key === 'Escape') adding = false;
			}}
			onblur={add}
			{@attach (node) => node.focus()}
		/>
	{:else}
		<button
			type="button"
			class="inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
			title="Tags group requests for filtering (tag:smoke) and for `plumbq run --tag`"
			onclick={() => (adding = true)}
		>
			<PlusIcon class="size-3" /> tag
		</button>
	{/if}
</div>
