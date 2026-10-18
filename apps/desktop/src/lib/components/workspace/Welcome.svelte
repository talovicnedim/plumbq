<script lang="ts">
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import FolderPlusIcon from '@lucide/svelte/icons/folder-plus';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import PlayIcon from '@lucide/svelte/icons/play';
	import logo from '#lib/assets/favicon.svg';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
</script>

<div data-tauri-drag-region class="flex h-full items-center justify-center bg-sidebar p-8">
	<div class="grid w-full max-w-lg gap-8">
		<div class="grid justify-items-center gap-3 text-center">
			<img src={logo} alt="" class="size-16" />
			<h1 class="text-2xl font-semibold tracking-tight">Plumbq</h1>
			<p class="max-w-sm text-sm text-muted-foreground">
				Requests are plain <code>.http</code> files in a folder: commit them, edit them in your editor,
				and let your AI assistant work in them too.
			</p>
		</div>

		{#if workspace.problem}
			<Alert.Root variant="destructive">
				<CircleAlertIcon />
				<Alert.Title>Couldn't load your workspaces</Alert.Title>
				<Alert.Description class="select-text">{workspace.problem}</Alert.Description>
				<Alert.Action>
					<Button size="sm" variant="outline" onclick={() => workspace.load()}>Try again</Button>
				</Alert.Action>
			</Alert.Root>
		{/if}

		<div class="grid grid-cols-2 gap-3">
			<button
				type="button"
				class="grid gap-1 rounded-xl border bg-background p-4 text-left shadow-xs transition hover:border-primary/60 hover:shadow-sm"
				onclick={() => workspace.create()}
			>
				<FolderPlusIcon class="mb-1 size-5 text-primary" />
				<span class="text-sm font-medium">New workspace</span>
				<span class="text-xs text-muted-foreground">Start fresh with an example request</span>
			</button>
			<button
				type="button"
				class="grid gap-1 rounded-xl border bg-background p-4 text-left shadow-xs transition hover:border-primary/60 hover:shadow-sm"
				onclick={() => workspace.choose()}
			>
				<FolderOpenIcon class="mb-1 size-5 text-primary" />
				<span class="text-sm font-medium">Open folder</span>
				<span class="text-xs text-muted-foreground"
					>Any folder with .http files, e.g. your repo</span
				>
			</button>
			<button
				type="button"
				class="col-span-2 flex items-center gap-3 rounded-xl border border-dashed bg-background p-4 text-left transition hover:border-primary/60"
				onclick={() => workspace.openDemo()}
			>
				<PlayIcon class="size-5 shrink-0 text-primary" />
				<span class="grid gap-0.5">
					<span class="text-sm font-medium">Try the demo workspace</span>
					<span class="text-xs text-muted-foreground">
						Seven ready-to-send requests against a public test API, with environments, chaining and
						assertions.
					</span>
				</span>
			</button>
		</div>

		{#if workspace.recent.length > 0}
			<div class="grid gap-1">
				<h2 class="px-1 text-xs font-medium text-muted-foreground">Recent</h2>
				<div class="overflow-hidden rounded-xl border bg-background">
					{#each workspace.recent as recent (recent.root)}
						<button
							type="button"
							class="flex w-full items-center gap-3 border-b px-4 py-2.5 text-left last:border-0 hover:bg-muted"
							onclick={() => workspace.open(recent.root)}
						>
							<FolderOpenIcon class="size-4 shrink-0 text-muted-foreground" />
							<span class="text-sm font-medium">{recent.name}</span>
							<span class="ml-auto truncate text-xs text-muted-foreground">{recent.root}</span>
						</button>
					{/each}
				</div>
			</div>
		{/if}

		<p class="text-center text-xs text-muted-foreground">Free. No account. No cloud.</p>
	</div>
</div>
