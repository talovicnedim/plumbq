<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { untrack } from 'svelte';
	import { api, toApiError, type Environment } from '#lib/api/index.js';
	import KeyValueEditor from '#lib/components/common/KeyValueEditor.svelte';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import EnvironmentList from './EnvironmentList.svelte';
	import EnvironmentTypeSelect from './EnvironmentTypeSelect.svelte';
	import OAuthClients from './OAuthClients.svelte';
	import SecretsEditor from './SecretsEditor.svelte';
	import WorkspaceDefaults from './WorkspaceDefaults.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { confirm } from '#lib/state/confirm.svelte.js';
	import { EnvironmentEditor } from '#lib/state/environment-editor.svelte.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { prompt } from '#lib/state/prompt.svelte.js';

	interface Props {
		open: boolean;
	}

	let { open = $bindable() }: Props = $props();

	const editor = new EnvironmentEditor();

	const variables = $derived(
		Object.entries(editor.draft?.variables ?? {}).map(([name, value]) => ({ name, value }))
	);

	$effect(() => {
		if (!open) return;
		untrack(() => editor.start(environments.list.data ?? [], environments.defaults.data));
	});

	$effect(() => {
		const list = environments.list.data;
		if (list) untrack(() => open && editor.follow(list));
	});

	$effect(() => {
		const defaults = environments.defaults.data;
		untrack(() => open && editor.followDefaults(defaults));
	});

	async function setOpen(next: boolean) {
		if (next || (await editor.mayClose())) open = next;
	}

	async function run(change: () => Promise<unknown>, success?: string) {
		try {
			await change();
			await environments.list.refresh();
			if (success) toast.success(success);
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async function create() {
		const name = await prompt({
			title: 'New environment',
			label: 'Name',
			confirmLabel: 'Create',
			placeholder: 'staging'
		});
		if (!name) return;
		try {
			await editor.create(name);
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async function remove(env: Environment) {
		const ok = await confirm({
			title: `Delete environment "${env.name}"?`,
			description: 'Its file and the secret values stored in this machine’s keychain are deleted.',
			confirmLabel: 'Delete',
			destructive: true
		});
		if (ok) await run(() => api.environments.remove(env.name));
	}
</script>

<Dialog.Root bind:open={() => open, (next) => void setOpen(next)}>
	<Dialog.Content class="sm:max-w-4xl">
		<Dialog.Header>
			<Dialog.Title>Environments</Dialog.Title>
			<Dialog.Description>
				Saved in <code>environments/*.toml</code> in your workspace. Secret values stay in the OS keychain.
			</Dialog.Description>
		</Dialog.Header>

		<QueryView query={environments.list}>
			{#snippet children(list)}
				<div class="grid h-[32rem] grid-cols-[12rem_1fr] gap-4">
					<EnvironmentList
						{list}
						skipped={environments.skipped}
						current={editor.draft?.name}
						showDefaults={editor.showDefaults}
						onselect={(name) => editor.select(name, list)}
						ondefaults={() => editor.openDefaults()}
						oncreate={create}
					/>

					{#if editor.showDefaults}
						<WorkspaceDefaults {editor} />
					{:else if editor.draft?.warning}
						<div
							class="grid content-start gap-2 rounded-md border border-destructive/40 bg-destructive/5 p-4 text-sm"
						>
							<p class="font-medium">This environment file can't be read</p>
							<p class="font-mono text-xs break-all text-muted-foreground select-text">
								{editor.draft.warning}
							</p>
							<p class="text-xs text-muted-foreground">
								Fix the file in your editor; it isn't changed from here until then.
							</p>
						</div>
					{:else if editor.draft}
						{@const env = editor.draft}
						<div class="grid min-h-0 content-start gap-4 overflow-y-auto">
							<EnvironmentTypeSelect bind:tag={env.tag} />
							<div class="grid gap-1.5 text-sm">
								Variables
								<KeyValueEditor
									rows={variables}
									onchange={(rows) => editor.setVariables(rows)}
									namePlaceholder="baseUrl"
									bulk="="
								/>
							</div>
							<SecretsEditor environment={env.name} bind:secrets={env.secrets} {run} />
							<OAuthClients
								bind:clients={env.auth}
								environment={env.name}
								onsave={() => api.environments.update(env)}
							/>
							<div class="flex justify-between">
								<Button variant="ghost" class="text-destructive" onclick={() => remove(env)}>
									Delete
								</Button>
								<div class="flex items-center gap-3">
									{#if editor.changedOnDisk}
										<span class="text-xs text-warning" title="Saving keeps your version">
											Changed on disk since you started editing
										</span>
									{/if}
									<Button
										onclick={() => run(() => api.environments.update(env), 'Environment saved')}
									>
										Save
									</Button>
								</div>
							</div>
						</div>
					{:else}
						<p class="py-12 text-center text-sm text-muted-foreground">
							No environments yet. Create one to use <code>{'{{variables}}'}</code> like a base URL.
						</p>
					{/if}
				</div>
			{/snippet}
		</QueryView>
	</Dialog.Content>
</Dialog.Root>
