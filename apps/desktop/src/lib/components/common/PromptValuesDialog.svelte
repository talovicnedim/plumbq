<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { promptValues } from '#lib/state/prompt-values.svelte.js';

	const request = $derived(promptValues.pending);
</script>

<Dialog.Root bind:open={() => request !== undefined, (open) => !open && promptValues.answer(null)}>
	<!-- Above other dialogs: the Runner asks for values while it is open. -->
	<Dialog.Content class="z-60 shadow-2xl sm:max-w-md">
		{#if request}
			<form
				class="grid gap-4"
				onsubmit={(e) => {
					e.preventDefault();
					promptValues.answer({ ...request.values });
				}}
			>
				<Dialog.Header>
					<Dialog.Title>{request.title}</Dialog.Title>
					<Dialog.Description>
						Asked by <code>{'{{$prompt …}}'}</code> variables. Values are only kept in memory until the
						app closes, and are masked in history.
					</Dialog.Description>
				</Dialog.Header>
				{#each request.names as name, i (name)}
					<label class="grid gap-1.5 text-sm">
						<code class="text-xs">{name}</code>
						<Input
							type="password"
							autocomplete="off"
							bind:value={() => request.values[name] ?? '', (v) => (request.values[name] = v)}
							autofocus={i === 0}
						/>
					</label>
				{/each}
				<Dialog.Footer class="gap-2">
					<Button type="button" variant="ghost" onclick={() => promptValues.answer(null)}>
						Cancel
					</Button>
					<Button type="submit">Send</Button>
				</Dialog.Footer>
			</form>
		{/if}
	</Dialog.Content>
</Dialog.Root>
