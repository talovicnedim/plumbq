import { untrack, type Component } from 'svelte';

/**
 * Items shown on the right of the status bar, registered by features that own them (e.g. the
 * MCP server indicator). `register` returns an unregister function, so use it in `$effect`:
 * `$effect(() => statusItems.register({ id: 'mcp', order: 10, component: McpStatus }))`.
 */
interface StatusItem {
	id: string;
	/** Lower comes first (left). */
	order: number;
	component: Component;
}

class StatusItemsState {
	items = $state<StatusItem[]>([]);

	register(item: StatusItem): () => void {
		// Called from `$effect`: reading `items` there would make the effect re-run on its own write.
		untrack(() => {
			this.items = [...this.items.filter((i) => i.id !== item.id), item].sort(
				(a, b) => a.order - b.order
			);
		});
		return () => {
			this.items = this.items.filter((i) => i.id !== item.id);
		};
	}
}

export const statusItems = new StatusItemsState();
