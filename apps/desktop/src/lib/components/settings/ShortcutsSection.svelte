<script lang="ts">
	import { SHORTCUTS, shortcutLabel, type ShortcutGroup } from '#lib/utils/shortcuts.js';

	const GROUPS: ShortcutGroup[] = ['Requests', 'Navigation', 'Tabs', 'App'];
</script>

{#each GROUPS as group (group)}
	<div class="py-2.5 not-first:border-t">
		<h3 class="mb-1.5 text-xs font-medium text-muted-foreground">{group}</h3>
		<dl class="grid gap-1 text-sm">
			{#each SHORTCUTS.filter((s) => s.group === group) as shortcut (shortcut.id)}
				<div class="flex justify-between">
					<dt>{shortcut.label}</dt>
					<dd>
						<kbd class="rounded border bg-muted px-1.5 font-mono text-xs"
							>{shortcutLabel(shortcut)}</kbd
						>
					</dd>
				</div>
			{/each}
		</dl>
	</div>
{/each}
