<script lang="ts">
	import { toast } from 'svelte-sonner';
	import {
		api,
		toApiError,
		type BodyComparison,
		type HistorySummary,
		type ResponseExample
	} from '#lib/api/index.js';
	import DiffView from '#lib/components/common/DiffView.svelte';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { confirm } from '#lib/state/confirm.svelte.js';
	import { Query } from '#lib/state/query.svelte.js';
	import type { RequestTab } from '#lib/state/tabs.svelte.js';
	import { formatTime } from '#lib/utils/format.js';
	import { prettyBody } from '#lib/utils/http.js';
	import CompareHeading from './CompareHeading.svelte';
	import CompareSources from './CompareSources.svelte';
	import FieldChanges from './FieldChanges.svelte';

	interface Props {
		open: boolean;
		tab: RequestTab;
		/** Preselect a saved example by name. */
		example?: string;
	}

	let { open = $bindable(), tab, example }: Props = $props();

	interface Baseline {
		key: string;
		label: string;
		detail: string;
		status: number;
		durationMs: number | null;
		body: string;
		exampleName?: string;
	}

	interface CompareLists {
		examples: ResponseExample[];
		history: HistorySummary[];
	}

	const sources = new Query<CompareLists>(async () => {
		const [examples, history] = await Promise.all([
			api.examples.list(tab.id),
			api.history.list(tab.id)
		]);
		return { examples, history };
	});
	let selected = $state<string>();
	let baseline = $state<Baseline>();
	let comparison = $state<BodyComparison>();
	/** Counts picks, so a slow earlier pick never replaces a newer one. */
	let picks = 0;

	const current = $derived(tab.entry?.response);
	const earlier = $derived((sources.data?.history ?? []).filter((h) => h.id !== tab.entry?.id));

	$effect(() => {
		if (!open) return;
		void load();
	});

	async function load() {
		await sources.refresh();
		const first = earlier[0];
		const examples = sources.data?.examples ?? [];
		selected = example
			? `example:${example}`
			: first
				? `history:${first.id}`
				: examples[0]
					? `example:${examples[0].name}`
					: undefined;
	}

	// Load the chosen baseline and compare it with the current response.
	$effect(() => {
		const key = selected;
		const after = current?.body ?? '';
		if (!key) return;
		void pick(key, after);
	});

	async function pick(key: string, after: string) {
		const pickNumber = ++picks;
		try {
			const next = await loadBaseline(key);
			if (!next) return;
			const result = await api.compare.bodies(next.body, after);
			if (pickNumber !== picks) return;
			baseline = next;
			comparison = result;
		} catch (err) {
			if (pickNumber === picks) toast.error(toApiError(err).message);
		}
	}

	async function loadBaseline(key: string): Promise<Baseline | undefined> {
		if (key.startsWith('example:')) {
			const found = sources.data?.examples.find((e) => `example:${e.name}` === key);
			if (!found) return undefined;
			return {
				key,
				label: found.name,
				detail: 'saved example',
				status: found.status,
				durationMs: null,
				body: found.body,
				exampleName: found.name
			};
		}
		const entry = await api.history.get(tab.id, key.slice('history:'.length));
		return {
			key,
			label: formatTime(entry.sentAt),
			detail: entry.environment ?? 'no environment',
			status: entry.response.status,
			durationMs: entry.response.durationMs,
			body: entry.response.body
		};
	}

	async function removeExample(name: string) {
		const ok = await confirm({
			title: `Delete example "${name}"?`,
			description: `It's removed from the request's .examples.json file.`,
			confirmLabel: 'Delete',
			destructive: true
		});
		if (!ok) return;
		try {
			await api.examples.remove(tab.id, name);
			if (selected === `example:${name}`) selected = undefined;
			await load();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	const isJson = $derived(comparison?.json ?? false);
	const pretty = (body: string) => (isJson ? prettyBody(body, 'application/json') : body);
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="flex h-[85vh] flex-col sm:max-w-6xl">
		<Dialog.Header>
			<Dialog.Title>Compare responses</Dialog.Title>
			<Dialog.Description>
				The current response (right) against a saved example or an earlier send (left).
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid min-h-0 flex-1 grid-cols-[15rem_1fr] gap-4">
			<div class="min-h-0 overflow-y-auto border-r pr-3">
				<QueryView query={sources}>
					{#snippet children(lists)}
						<CompareSources
							examples={lists.examples}
							history={earlier}
							bind:selected
							onremove={removeExample}
						/>
					{/snippet}
				</QueryView>
			</div>

			<div class="flex min-h-0 flex-col gap-3">
				{#if !current}
					<p class="py-12 text-center text-sm text-muted-foreground">
						Send the request first to compare its response.
					</p>
				{:else if baseline && comparison}
					<CompareHeading {baseline} {current} />
					<FieldChanges {comparison} />
					{#key baseline.key + current.body}
						<DiffView
							before={pretty(baseline.body)}
							after={pretty(current.body)}
							json={isJson}
							class="flex-1"
						/>
					{/key}
				{:else}
					<p class="py-12 text-center text-sm text-muted-foreground">
						Pick something on the left to compare with.
					</p>
				{/if}
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>
