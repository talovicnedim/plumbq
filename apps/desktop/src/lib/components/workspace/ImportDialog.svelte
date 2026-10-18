<script lang="ts">
	import FileUpIcon from '@lucide/svelte/icons/file-up';
	import FolderInputIcon from '@lucide/svelte/icons/folder-input';
	import LinkIcon from '@lucide/svelte/icons/link';
	import TerminalIcon from '@lucide/svelte/icons/terminal';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { importer } from '#lib/state/importer.svelte.js';
	import { cn } from '#lib/utils.js';

	let command = $state('');
	let name = $state('');
	let specUrl = $state('');

	// Core checks the URL and the command and explains what's wrong.
	const hasUrl = $derived(specUrl.trim() !== '');
	const hasCommand = $derived(command.trim() !== '');
	/** A name from the URL's last path segment until the user types one. */
	const suggestedName = $derived.by(() => {
		const url = command.match(/https?:\/\/[^\s'"]+/)?.[0];
		const segment = url?.split('?')[0].split('/').filter(Boolean).pop();
		return segment && !segment.includes('.') ? segment.replace(/[-_]/g, ' ') : 'Imported request';
	});

	// Each time the dialog opens, start clean.
	$effect(() => {
		if (!importer.open) return;
		command = '';
		name = '';
		specUrl = '';
	});

	async function importCurl() {
		await importer.curl(name.trim() || suggestedName, command);
	}
</script>

<Dialog.Root bind:open={importer.open}>
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>Import</Dialog.Title>
			<Dialog.Description>
				Into <code>{importer.folder || 'the workspace root'}</code>. Everything becomes plain
				<code>.http</code> files you can edit and commit.
			</Dialog.Description>
		</Dialog.Header>

		<div class="grid grid-cols-2 gap-2">
			{#each [{ id: 'curl', label: 'cURL command', hint: 'Paste from a terminal, docs or browser dev tools', icon: TerminalIcon }, { id: 'file', label: 'Collection file', hint: 'Postman, OpenAPI, Swagger, Insomnia, HAR or Bruno', icon: FileUpIcon }] as option (option.id)}
				<button
					type="button"
					class={cn(
						'flex items-start gap-3 rounded-lg border p-3 text-left transition-colors',
						importer.source === option.id ? 'border-primary bg-primary/5' : 'hover:bg-muted'
					)}
					onclick={() => (importer.source = option.id === 'curl' ? 'curl' : 'file')}
				>
					<option.icon class="mt-0.5 size-4 shrink-0 text-primary" />
					<span class="grid gap-0.5">
						<span class="text-sm font-medium">{option.label}</span>
						<span class="text-xs text-muted-foreground">{option.hint}</span>
					</span>
				</button>
			{/each}
		</div>

		{#if importer.source === 'curl'}
			<form
				class="grid gap-3"
				onsubmit={(e) => {
					e.preventDefault();
					if (hasCommand) void importCurl();
				}}
			>
				<Textarea
					bind:value={command}
					class="h-40 font-mono text-xs"
					placeholder={"curl 'https://api.example.com/users' -H 'accept: application/json'"}
					autofocus
				/>
				<div class="flex items-end gap-2">
					<label class="grid flex-1 gap-1.5 text-sm">
						Name
						<Input bind:value={name} placeholder={suggestedName} />
					</label>
					<Button type="submit" disabled={!hasCommand || importer.busy}>Import request</Button>
				</div>
				<p class="text-xs text-muted-foreground">
					Tip: you can also paste a curl command straight into any request's URL bar.
				</p>
			</form>
		{:else}
			<div class="grid gap-3">
				<div class="grid grid-cols-2 gap-2">
					<button
						type="button"
						class="flex flex-col items-center gap-2 rounded-lg border-2 border-dashed p-6 text-sm text-muted-foreground hover:border-primary/60 hover:bg-primary/5"
						disabled={importer.busy}
						onclick={() => importer.browse()}
					>
						<FileUpIcon class="size-6 text-primary" />
						<span class="font-medium text-foreground">Choose a file…</span>
						<span class="text-center text-xs">
							Postman, OpenAPI, Swagger (JSON or YAML), Insomnia, HAR or a single .bru request
						</span>
					</button>
					<button
						type="button"
						class="flex flex-col items-center gap-2 rounded-lg border-2 border-dashed p-6 text-sm text-muted-foreground hover:border-primary/60 hover:bg-primary/5"
						disabled={importer.busy}
						onclick={() => importer.browseFolder()}
					>
						<FolderInputIcon class="size-6 text-primary" />
						<span class="font-medium text-foreground">Choose a Bruno folder…</span>
						<span class="text-center text-xs">
							The collection folder with bruno.json or opencollection.yml
						</span>
					</button>
				</div>
				<form
					class="flex items-end gap-2"
					onsubmit={(e) => {
						e.preventDefault();
						if (hasUrl) void importer.url(specUrl.trim());
					}}
				>
					<label class="grid flex-1 gap-1.5 text-sm">
						<span class="flex items-center gap-1.5"
							><LinkIcon class="size-3.5" /> Or an OpenAPI URL</span
						>
						<Input
							bind:value={specUrl}
							placeholder="https://petstore3.swagger.io/api/v3/openapi.json"
							class="font-mono text-xs"
						/>
					</label>
					<Button type="submit" disabled={!hasUrl || importer.busy}>Import</Button>
				</form>
				<p class="text-xs text-muted-foreground">
					Importing the same collection again updates it: new requests are added, nothing you edited
					is overwritten, and nothing is deleted. You can also drop files or a Bruno folder anywhere
					on the window.
				</p>
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
