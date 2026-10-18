<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';

	interface Props {
		open: boolean;
		title: string;
		description: string;
		submitLabel: string;
		/** Disables the submit button until the form is valid. */
		valid: boolean;
		/** Return true to close the dialog (e.g. when the action ran). */
		onsubmit: () => Promise<boolean>;
		children: Snippet;
	}

	let {
		open = $bindable(),
		title,
		description,
		submitLabel,
		valid,
		onsubmit,
		children
	}: Props = $props();

	let busy = $state(false);

	async function submit() {
		busy = true;
		try {
			if (await onsubmit()) open = false;
		} finally {
			busy = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-2xl">
		<form
			class="grid gap-4"
			onsubmit={(e) => {
				e.preventDefault();
				if (valid && !busy) void submit();
			}}
		>
			<Dialog.Header>
				<Dialog.Title>{title}</Dialog.Title>
				<Dialog.Description>{description}</Dialog.Description>
			</Dialog.Header>

			{@render children()}

			<Dialog.Footer class="gap-2">
				<Button type="button" variant="ghost" onclick={() => (open = false)}>Cancel</Button>
				<Button type="submit" disabled={!valid || busy}>{submitLabel}</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
