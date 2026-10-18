<script lang="ts">
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import ApprovalForm from './ApprovalForm.svelte';

	const request = $derived(ai.pending[0]);

	function deny() {
		if (request) void ai.answer(request.id, 'deny', null);
	}
</script>

<Dialog.Root open={request !== undefined} onOpenChange={(open) => !open && deny()}>
	<Dialog.Content class="z-60 max-h-[92vh] overflow-y-auto shadow-2xl sm:max-w-3xl">
		{#if request}
			<!-- A new request (the last one answered, cancelled or timed out) starts with nothing typed. -->
			{#key request.id}
				<ApprovalForm {request} waiting={ai.pending.length - 1} />
			{/key}
		{/if}
	</Dialog.Content>
</Dialog.Root>
