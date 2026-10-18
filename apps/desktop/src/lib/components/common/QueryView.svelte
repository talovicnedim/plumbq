<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import type { Query } from '#lib/state/query.svelte.js';

	interface Props {
		query: Query<T>;
		/** Text shown when `isEmpty(data)` is true. */
		empty?: string;
		isEmpty?: (data: T) => boolean;
		children: Snippet<[T]>;
	}

	let { query, empty = 'Nothing here yet.', isEmpty, children }: Props = $props();
</script>

{#if query.error}
	<Alert.Root variant="destructive" class="mb-4">
		<CircleAlertIcon />
		<Alert.Title>Couldn't load data</Alert.Title>
		<Alert.Description>{query.error.message}</Alert.Description>
		<Alert.Action>
			<Button size="sm" variant="outline" onclick={() => query.refresh()}>Retry</Button>
		</Alert.Action>
	</Alert.Root>
{/if}

{#if query.data !== undefined}
	{#if isEmpty?.(query.data)}
		<p class="py-12 text-center text-sm text-muted-foreground">{empty}</p>
	{:else}
		{@render children(query.data)}
	{/if}
{:else if !query.error}
	<div class="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground">
		<LoaderIcon class="size-4 animate-spin" /> Loading…
	</div>
{/if}
