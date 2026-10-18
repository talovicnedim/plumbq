<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Textarea } from '#lib/components/ui/textarea/index.js';
	import { promptState } from '#lib/state/prompt.svelte.js';

	const request = $derived(promptState.pending);
	// Each new prompt starts from its initial value; typing overrides it.
	let value = $derived(request?.initial ?? '');

	function answer(result: string | null) {
		promptState.answer(result);
	}
</script>

<Dialog.Root bind:open={() => request !== undefined, (open) => !open && answer(null)}>
	<Dialog.Content class={request?.multiline ? 'sm:max-w-2xl' : 'sm:max-w-md'}>
		{#if request}
			<form
				class="grid gap-4"
				onsubmit={(e) => {
					e.preventDefault();
					if (value.trim()) answer(value.trim());
				}}
			>
				<Dialog.Header>
					<Dialog.Title>{request.title}</Dialog.Title>
				</Dialog.Header>
				<label class="grid gap-1.5 text-sm">
					{request.label}
					{#if request.multiline}
						<Textarea
							bind:value
							placeholder={request.placeholder}
							class="h-48 font-mono text-xs"
							autofocus
						/>
					{:else}
						<Input bind:value placeholder={request.placeholder} autocomplete="off" autofocus />
					{/if}
				</label>
				<Dialog.Footer class="gap-2">
					<Button type="button" variant="ghost" onclick={() => answer(null)}>Cancel</Button>
					<Button type="submit" disabled={!value.trim()}>{request.confirmLabel}</Button>
				</Dialog.Footer>
			</form>
		{/if}
	</Dialog.Content>
</Dialog.Root>
