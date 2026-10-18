<script lang="ts">
	import { api, toApiError, type Response } from '#lib/api/index.js';
	import CodeEditor from '#lib/components/common/CodeEditor.svelte';
	import {
		countLabel,
		findMatches,
		searchMode,
		stepIndex,
		type Matches
	} from '#lib/utils/body-search.js';
	import { prettyBody } from '#lib/utils/http.js';
	import { previewKind, type BodyMode } from '#lib/utils/preview.js';
	import { bodySearch } from './body-search.js';
	import ResponsePreview from './ResponsePreview.svelte';
	import ResponseSearchBox from './ResponseSearchBox.svelte';

	interface Props {
		response: Response;
		contentType: string | undefined;
		mode: BodyMode;
		/** Wrap long lines in the text view. */
		wrap: boolean;
		/** Changes per response, so editors are recreated instead of patched. */
		key: string;
	}

	let { response, contentType, mode, wrap, key }: Props = $props();

	const NO_MATCHES: Matches = { ranges: [], capped: false };

	let input = $state('');
	let current = $state(0);
	let filtered = $state<string>();
	let filterError = $state<string>();
	let box = $state<ResponseSearchBox>();

	const kind = $derived(previewKind(contentType));
	const isJson = $derived(contentType?.includes('json') ?? /^\s*[[{]/.test(response.body));
	const text = $derived(mode === 'pretty' ? prettyBody(response.body, contentType) : response.body);
	const search = $derived(searchMode(input, isJson));
	const matches = $derived(search.kind === 'find' ? findMatches(text, search.query) : NO_MATCHES);
	const count = $derived(matches.ranges.length);
	// A new body can have fewer matches than the one the position was set on.
	const index = $derived(Math.min(current, Math.max(count - 1, 0)));
	const extensions = $derived(bodySearch(matches.ranges, index, () => box?.focus()));

	// Filtering runs in core (the same JSON path as assertions and chaining), debounced.
	$effect(() => {
		const path = search.kind === 'path' ? search.path : '';
		const body = response.body;
		filtered = undefined;
		filterError = undefined;
		if (!path || path === '$') return;
		// A slow answer for an older filter or body must not replace the newer one.
		let stale = false;
		const timer = setTimeout(() => {
			api.jsonPath.query(body, path).then(
				(result) => !stale && (filtered = result),
				(err) => !stale && (filterError = toApiError(err).message)
			);
		}, 200);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	});

	function step(direction: 1 | -1) {
		current = stepIndex(index, count, direction);
	}
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
	{#if mode === 'preview' && kind && (response.binary ? response.bodyBase64 : response.body)}
		<ResponsePreview {response} {kind} contentType={contentType ?? ''} />
	{:else if response.binary}
		<p class="py-8 text-center text-sm text-muted-foreground">
			{response.bodyBase64
				? `Binary response (${contentType ?? 'unknown type'}). Save it to a file to open it.`
				: `Binary response (${contentType ?? 'unknown type'}), too large to keep in history.`}
		</p>
	{:else if !response.body}
		<p class="py-8 text-center text-sm text-muted-foreground">Empty body.</p>
	{:else}
		<ResponseSearchBox
			bind:this={box}
			bind:value={() => input, (value) => ((input = value), (current = 0))}
			placeholder={isJson ? 'Search, or filter with $.items[*].name' : 'Search'}
			status={search.kind === 'find' ? countLabel(index, count, matches.capped) : undefined}
			error={filterError}
			canStep={count > 1}
			onstep={step}
		/>
		{#key `${key}:${isJson}:${mode}:${filtered !== undefined}:${wrap}`}
			<CodeEditor
				value={filtered ?? text}
				json={isJson}
				readonly
				{wrap}
				{extensions}
				class="min-h-0 flex-1"
			/>
		{/key}
	{/if}
</div>
