<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import { newRequest } from '#lib/state/actions.js';
	import { overlays } from '#lib/state/overlays.svelte.js';
	import { palette } from '#lib/state/palette.svelte.js';
	import { settings } from '#lib/state/settings.svelte.js';
	import { labelFor } from '#lib/utils/shortcuts.js';

	const TIPS = [
		{
			title: 'Find anything',
			text: `${labelFor('palette')} searches every request by name, method, URL, folder or tag, and runs commands.`
		},
		{
			title: 'Paste a curl command',
			text: 'Paste `curl …` into any URL bar and it becomes the request: method, headers and body.'
		},
		{
			title: 'Let your AI work here',
			text: 'Connect Claude, Cursor or Copilot: they can organize, write and send requests, with your approval.'
		},
		{
			title: 'Run a whole folder',
			text: 'Right-click a folder → Run folder, with assertions, CSV data and a results table.'
		}
	];
</script>

<div
	class="flex flex-1 flex-col items-center justify-center gap-6 p-8 text-sm text-muted-foreground"
>
	<p>Open a request from the sidebar, or start a new one.</p>
	<div class="flex gap-2">
		<button
			type="button"
			class="rounded-md border px-3 py-1.5 hover:bg-muted"
			onclick={() => newRequest('')}
		>
			New request <kbd class="ml-1 text-[10px]">{labelFor('newRequest')}</kbd>
		</button>
		<button
			type="button"
			class="rounded-md border px-3 py-1.5 hover:bg-muted"
			onclick={() => (palette.open = true)}
		>
			Search <kbd class="ml-1 text-[10px]">{labelFor('palette')}</kbd>
		</button>
		<button
			type="button"
			class="rounded-md border px-3 py-1.5 hover:bg-muted"
			onclick={() => (overlays.recent = true)}
		>
			Recent <kbd class="ml-1 text-[10px]">{labelFor('recent')}</kbd>
		</button>
	</div>

	{#if settings.values.showTips}
		<section
			class="relative w-full max-w-xl rounded-xl border bg-background p-4 text-left shadow-xs"
		>
			<button
				type="button"
				class="absolute top-2 right-2 rounded p-1 hover:bg-muted"
				aria-label="Hide tips"
				title="Hide tips (Settings brings them back)"
				onclick={() => settings.update({ showTips: false })}
			>
				<XIcon class="size-3.5" />
			</button>
			<h2 class="mb-3 text-xs font-semibold tracking-wider uppercase">Tips</h2>
			<ul class="grid gap-3 sm:grid-cols-2">
				{#each TIPS as tip (tip.title)}
					<li>
						<p class="font-medium text-foreground">{tip.title}</p>
						<p class="text-xs">{tip.text}</p>
					</li>
				{/each}
			</ul>
		</section>
	{/if}
</div>
