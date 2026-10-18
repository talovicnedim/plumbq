<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import { toast } from 'svelte-sonner';
	import { api, toApiError, type Cookie } from '#lib/api/index.js';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import { confirm } from '#lib/state/confirm.svelte.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { Query } from '#lib/state/query.svelte.js';
	import { formatExpiry } from '#lib/utils/cookies.js';

	interface Props {
		/** Changes after every send, so the jar is re-read when responses set cookies. */
		reloadKey: string | undefined;
	}

	let { reloadKey }: Props = $props();

	const jar = new Query(() => api.cookies.list(environments.selected));

	const label = $derived(environments.selected ?? 'no environment');

	$effect(() => {
		void reloadKey;
		void environments.selected;
		void jar.refresh();
	});

	async function remove(cookie: Cookie) {
		const ok = await confirm({
			title: `Delete cookie ${cookie.name}?`,
			description: `Requests to ${cookie.domain} stop sending it, which may log you out there.`,
			confirmLabel: 'Delete',
			destructive: true
		});
		if (!ok) return;
		try {
			await api.cookies.remove(environments.selected, cookie.domain, cookie.name);
			await jar.refresh();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async function clear() {
		const ok = await confirm({
			title: `Clear cookies for ${label}?`,
			description: 'Every stored cookie of this environment is deleted, e.g. to log out.',
			confirmLabel: 'Clear cookies',
			destructive: true
		});
		if (!ok) return;
		try {
			await api.cookies.clear(environments.selected);
			await jar.refresh();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}
</script>

<div class="grid gap-2">
	<div class="flex items-center justify-between text-xs text-muted-foreground">
		<span>
			Cookie jar for <span class="font-medium text-foreground">{label}</span>. Set by responses,
			sent with matching requests. Kept on this machine only.
		</span>
		{#if (jar.data ?? []).length > 0}
			<button type="button" class="text-destructive hover:underline" onclick={clear}
				>Clear all</button
			>
		{/if}
	</div>
	<QueryView query={jar} empty="No cookies stored yet." isEmpty={(list) => list.length === 0}>
		{#snippet children(list)}
			<table class="w-full font-mono text-xs select-text">
				<thead class="text-left text-muted-foreground">
					<tr class="border-b">
						<th class="py-1.5 pr-3 font-normal">Name</th>
						<th class="py-1.5 pr-3 font-normal">Value</th>
						<th class="py-1.5 pr-3 font-normal">Domain</th>
						<th class="py-1.5 pr-3 font-normal">Path</th>
						<th class="py-1.5 pr-3 font-normal">Expires</th>
						<th></th>
					</tr>
				</thead>
				<tbody>
					{#each list as cookie (`${cookie.domain}|${cookie.path}|${cookie.name}`)}
						<tr class="border-b last:border-0">
							<td class="py-1.5 pr-3">{cookie.name}</td>
							<td class="max-w-48 truncate py-1.5 pr-3" title={cookie.value}>{cookie.value}</td>
							<td class="py-1.5 pr-3">{cookie.hostOnly ? cookie.domain : `.${cookie.domain}`}</td>
							<td class="py-1.5 pr-3">{cookie.path}</td>
							<td class="py-1.5 pr-3 font-sans">
								{formatExpiry(cookie.expires)}{cookie.secure ? ' · secure' : ''}
							</td>
							<td class="py-1.5 text-right">
								<button
									type="button"
									class="text-muted-foreground hover:text-foreground"
									aria-label={`Delete cookie ${cookie.name}`}
									onclick={() => remove(cookie)}
								>
									<XIcon class="size-3.5" />
								</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/snippet}
	</QueryView>
</div>
