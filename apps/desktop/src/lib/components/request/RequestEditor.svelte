<script lang="ts">
	import { environments } from '#lib/state/environments.svelte.js';
	import { RequestViewState } from '#lib/state/request-view.svelte.js';
	import type { RequestTab } from '#lib/state/tabs.svelte.js';
	import RequestParts from './RequestParts.svelte';
	import RequestTitle from './RequestTitle.svelte';
	import RequestUrlBar from './RequestUrlBar.svelte';
	import TagEditor from './TagEditor.svelte';
	import VariableHints from './VariableHints.svelte';

	interface Props {
		tab: RequestTab;
		onmanageEnvironments: () => void;
	}

	let { tab, onmanageEnvironments }: Props = $props();

	const view = new RequestViewState();

	// Core looks at the draft again after edits and when the environment or its file changes.
	$effect(() => {
		void JSON.stringify([tab.id, tab.draft, environments.current]);
		view.refresh(tab);
		return () => view.stop();
	});
</script>

<div class="flex h-full min-h-0 flex-col">
	{#if environments.current?.tag === 'production'}
		<div
			class="flex items-center gap-2 border-b border-env-production/30 bg-env-production/10 px-4 py-1.5 text-xs"
		>
			<span class="size-1.5 rounded-full bg-env-production"></span>
			<span class="font-semibold text-env-production">Production: {environments.current.name}.</span
			>
			<span class="text-muted-foreground">Changes need confirmation.</span>
		</div>
	{/if}

	<section class="flex min-h-0 flex-1 flex-col gap-3 p-4">
		<RequestTitle {tab} />
		<RequestUrlBar {tab} variables={view.data?.variables ?? []} />

		<VariableHints
			variables={view.data?.variables ?? []}
			error={view.error}
			environment={environments.selected}
			onmanage={onmanageEnvironments}
		/>

		<TagEditor
			tags={view.data?.tags ?? []}
			comments={tab.draft.comments}
			oncomments={(c) => (tab.draft.comments = c)}
		/>
		<RequestParts {tab} view={view.data} />
	</section>
</div>
