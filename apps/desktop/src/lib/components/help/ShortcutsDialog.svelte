<script lang="ts">
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { overlays } from '#lib/state/overlays.svelte.js';
	import { isMac } from '#lib/utils/platform.js';
	import { SHORTCUTS, shortcutLabel, type ShortcutGroup } from '#lib/utils/shortcuts.js';

	const GROUPS: ShortcutGroup[] = ['Requests', 'Navigation', 'Tabs', 'App'];
	/** Editor and tree keys that aren't app shortcuts but are worth knowing. */
	const EXTRAS: { group: string; keys: string; label: string }[] = [
		{ group: 'Sidebar', keys: '↑ ↓', label: 'Move between requests and folders' },
		{ group: 'Sidebar', keys: '→ ←', label: 'Expand or collapse a folder' },
		{ group: 'Sidebar', keys: '↵', label: 'Open the request' },
		{ group: 'Sidebar', keys: 'F2', label: 'Rename' },
		{ group: 'Sidebar', keys: 'Delete', label: 'Delete (asks first)' },
		{ group: 'Editors', keys: isMac ? '⌘F' : 'Ctrl+F', label: 'Find in the body or response' }
	];
</script>

<Dialog.Root bind:open={overlays.shortcuts}>
	<Dialog.Content class="sm:max-w-2xl">
		<Dialog.Header>
			<Dialog.Title>Keyboard shortcuts</Dialog.Title>
			<Dialog.Description>Everything works from the keyboard.</Dialog.Description>
		</Dialog.Header>
		<div class="grid max-h-[65vh] grid-cols-2 gap-x-8 gap-y-5 overflow-y-auto text-sm">
			{#each GROUPS as group (group)}
				<section>
					<h3 class="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
						{group}
					</h3>
					<dl class="grid gap-1.5">
						{#each SHORTCUTS.filter((s) => s.group === group) as shortcut (shortcut.id)}
							<div class="flex items-center justify-between gap-4">
								<dt>{shortcut.label}</dt>
								<dd>
									<kbd class="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs"
										>{shortcutLabel(shortcut)}</kbd
									>
								</dd>
							</div>
						{/each}
					</dl>
				</section>
			{/each}
			{#each ['Sidebar', 'Editors'] as group (group)}
				<section>
					<h3 class="mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
						{group}
					</h3>
					<dl class="grid gap-1.5">
						{#each EXTRAS.filter((e) => e.group === group) as extra (extra.label)}
							<div class="flex items-center justify-between gap-4">
								<dt>{extra.label}</dt>
								<dd>
									<kbd class="rounded border bg-muted px-1.5 py-0.5 font-mono text-xs"
										>{extra.keys}</kbd
									>
								</dd>
							</div>
						{/each}
					</dl>
				</section>
			{/each}
		</div>
	</Dialog.Content>
</Dialog.Root>
