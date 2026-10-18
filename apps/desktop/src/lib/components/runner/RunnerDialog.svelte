<script lang="ts">
	import DownloadIcon from '@lucide/svelte/icons/download';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import PlayIcon from '@lucide/svelte/icons/play';
	import SquareIcon from '@lucide/svelte/icons/square';
	import XIcon from '@lucide/svelte/icons/x';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { runner } from '#lib/state/runner.svelte.js';
	import { baseName, formatDuration } from '#lib/utils/format.js';
	import { sentCount } from '#lib/utils/runner.js';
	import RunOutcomeRow from './RunOutcomeRow.svelte';

	const requests = $derived(runner.preview?.requests);
	const results = $derived(runner.results);
	const total = $derived(
		requests === undefined
			? undefined
			: requests * (runner.dataFile ? 0 : Math.max(1, runner.iterations))
	);
	const showIteration = $derived(results.outcomes.some((o) => o.iteration > 1));
</script>

<Dialog.Root bind:open={runner.open}>
	<Dialog.Content class="flex h-[85vh] flex-col sm:max-w-5xl">
		<Dialog.Header>
			<Dialog.Title>Run {runner.target ? runner.target : 'all requests'}</Dialog.Title>
			<Dialog.Description>
				Sends every request in sidebar order and checks its assertions
				{#if environments.selected}with <span class="font-medium">{environments.selected}</span
					>{/if}.
				{#if requests !== undefined}{requests} request{requests === 1 ? '' : 's'}.{/if}
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid grid-cols-[1fr_1fr_1fr_auto] items-end gap-3 text-sm">
			<label class="grid gap-1.5">
				Tags
				<Input
					bind:value={runner.tags}
					placeholder="smoke, auth"
					onchange={() => runner.refreshPreview()}
					disabled={runner.running}
				/>
			</label>
			<label class="grid gap-1.5">
				Delay between requests (ms)
				<Input type="number" min="0" bind:value={runner.delayMs} disabled={runner.running} />
			</label>
			<label class="grid gap-1.5">
				Iterations
				<Input
					type="number"
					min="1"
					bind:value={runner.iterations}
					disabled={runner.running || runner.dataFile !== null}
				/>
			</label>
			<label class="flex h-9 items-center gap-2">
				<Switch bind:checked={runner.bail} disabled={runner.running} /> Stop at first failure
			</label>
		</div>

		<div class="flex items-center gap-2 text-sm">
			<span class="text-muted-foreground">Data file</span>
			{#if runner.dataFile}
				<code class="truncate rounded bg-muted px-1.5 py-0.5 text-xs" title={runner.dataFile}>
					{baseName(runner.dataFile)}
				</code>
				<button
					type="button"
					class="text-muted-foreground hover:text-foreground"
					aria-label="Remove data file"
					onclick={() => (runner.dataFile = null)}
				>
					<XIcon class="size-3.5" />
				</button>
				<span class="text-xs text-muted-foreground">one run per row, columns are variables</span>
			{:else}
				<Button
					size="sm"
					variant="ghost"
					class="h-7"
					onclick={() => runner.chooseData()}
					disabled={runner.running}
				>
					Choose CSV or JSON…
				</Button>
			{/if}
			<div class="ml-auto flex gap-2">
				{#if runner.report && !runner.running}
					<Button size="sm" variant="outline" onclick={() => runner.exportReport()}>
						<DownloadIcon /> Save results
					</Button>
				{/if}
				{#if runner.running}
					<Button size="sm" variant="destructive" onclick={() => runner.stop()}>
						<SquareIcon /> Stop
					</Button>
				{:else}
					<Button size="sm" onclick={() => runner.start()} disabled={requests === 0}>
						<PlayIcon /> Run
					</Button>
				{/if}
			</div>
		</div>

		<div class="min-h-0 flex-1 overflow-y-auto rounded-lg border">
			<table class="w-full text-sm">
				<thead
					class="sticky top-0 bg-muted/80 text-left text-xs text-muted-foreground backdrop-blur"
				>
					<tr>
						<th class="w-8 px-3 py-2"></th>
						{#if showIteration}<th class="px-2 py-2">#</th>{/if}
						<th class="px-2 py-2">Request</th>
						<th class="px-2 py-2">Status</th>
						<th class="px-2 py-2">Time</th>
						<th class="px-2 py-2">Tests</th>
					</tr>
				</thead>
				<tbody>
					{#each results.outcomes as outcome, i (i)}
						<RunOutcomeRow {outcome} {showIteration} />
					{:else}
						<tr>
							<td colspan="6" class="py-12 text-center text-sm text-muted-foreground">
								{runner.running ? 'Starting…' : 'Press Run to send the requests.'}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<div class="flex items-center gap-4 text-sm">
			{#if runner.running}
				<LoaderIcon class="size-4 animate-spin text-muted-foreground" />
				<span class="text-muted-foreground">
					{sentCount(results)}{total ? ` / ${total}` : ''} sent…
				</span>
			{/if}
			<span class="text-success">{results.passed} passed</span>
			<span class={results.failed ? 'text-destructive' : 'text-muted-foreground'}
				>{results.failed} failed</span
			>
			<span class="text-muted-foreground">{results.skipped} skipped</span>
			{#if results.dropped > 0}
				<span class="text-muted-foreground">({results.dropped} more not listed)</span>
			{/if}
			{#if runner.report}
				<span class="ml-auto text-muted-foreground">{formatDuration(runner.report.durationMs)}</span
				>
			{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
