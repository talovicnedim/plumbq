<script lang="ts">
	import HistoryIcon from '@lucide/svelte/icons/history';
	import XIcon from '@lucide/svelte/icons/x';
	import QueryView from '#lib/components/common/QueryView.svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
		aria-label="Recent chats"
		title="Recent chats"
	>
		<HistoryIcon class="size-4" />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="max-h-80 w-72 overflow-y-auto">
		<QueryView
			query={assistant.chats}
			empty="No chats in this workspace yet."
			isEmpty={(chats) => chats.length === 0}
		>
			{#snippet children(chats)}
				{#each chats as chat (chat.id)}
					<DropdownMenu.Item class="group gap-2" onclick={() => assistant.openChat(chat.id)}>
						<span class="truncate">{chat.title}</span>
						<button
							type="button"
							class="ml-auto opacity-0 group-hover:opacity-100"
							aria-label="Delete chat"
							onclick={(e) => {
								e.stopPropagation();
								void assistant.removeChat(chat.id, chat.title);
							}}
						>
							<XIcon class="size-3.5" />
						</button>
					</DropdownMenu.Item>
				{/each}
			{/snippet}
		</QueryView>
	</DropdownMenu.Content>
</DropdownMenu.Root>
