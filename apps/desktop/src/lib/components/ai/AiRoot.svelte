<script lang="ts">
	import { onApproval } from '#lib/api/index.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
	import { statusItems } from '#lib/state/status-items.svelte.js';
	import AiDialog from './AiDialog.svelte';
	import ApprovalDialog from './ApprovalDialog.svelte';
	import McpStatus from './McpStatus.svelte';

	// Always mounted (root layout): approvals must show wherever the user is.
	$effect(() => {
		void ai.load();
		// "Turn AI off" comes from ai.load. The chat's own settings tell `ready` whether a
		// question can go out right away.
		void assistant.load();
	});

	$effect(() => onApproval(() => void ai.refreshPending()));

	$effect(() => statusItems.register({ id: 'mcp', order: 10, component: McpStatus }));
</script>

<ApprovalDialog />
<AiDialog />
