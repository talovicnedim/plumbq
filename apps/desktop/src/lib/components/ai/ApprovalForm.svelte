<script lang="ts">
	import ShieldAlertIcon from '@lucide/svelte/icons/shield-alert';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import type { ApprovalAnswer, ApprovalRequest } from '#lib/api/index.js';
	import EnvBadge from '#lib/components/common/EnvBadge.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { cn } from '#lib/utils.js';
	import { canAllow } from '#lib/utils/approvals.js';

	interface Props {
		request: ApprovalRequest;
		/** How many more requests wait behind this one. */
		waiting: number;
	}

	let { request, waiting }: Props = $props();

	let typed = $state('');
	let busy = $state(false);

	const allowed = $derived(canAllow(request, typed));
	const actor = $derived(request.actorLabel || request.actor || 'An AI app');
	/** The environment's tag, when the request is for the workspace open here. */
	const tag = $derived(
		request.workspace === workspace.root
			? environments.list.data?.find((env) => env.name === request.environment)?.tag
			: undefined
	);

	async function answer(choice: ApprovalAnswer) {
		busy = true;
		await ai.answer(request.id, choice, choice === 'deny' ? null : typed);
		busy = false;
	}
</script>

<form
	class="grid min-w-0 gap-4"
	onsubmit={(e) => {
		e.preventDefault();
		if (allowed && !busy) void answer('allowOnce');
	}}
>
	<Dialog.Header>
		<Dialog.Title class="flex items-start gap-2 leading-snug">
			<ShieldAlertIcon
				class={cn(
					'mt-0.5 size-5 shrink-0',
					request.production ? 'text-env-production' : 'text-primary'
				)}
			/>
			<span class="min-w-0 wrap-anywhere">
				<span class="inline-block max-w-56 truncate align-bottom">{actor}</span>
				wants to {request.action}
			</span>
		</Dialog.Title>
		<Dialog.Description class="flex flex-wrap items-center gap-x-1.5 gap-y-1">
			{#if request.production}
				<span class="font-semibold text-env-production">Production.</span>
			{/if}
			<span>
				Workspace <span class="font-medium text-foreground"
					>{request.workspaceName || request.workspace}</span
				>{#if request.environment}, environment
					<span class="font-medium text-foreground">{request.environment}</span>{/if}
			</span>
			{#if tag}<EnvBadge {tag} />{/if}
			{#if waiting > 0}<span class="ml-auto">{waiting} more waiting</span>{/if}
		</Dialog.Description>
	</Dialog.Header>

	{#if request.reason}
		<div class="rounded-md border bg-muted/40 px-3 py-2.5 text-sm">
			<p class="mb-0.5 text-xs font-medium text-muted-foreground">Why</p>
			<p class="wrap-anywhere whitespace-pre-wrap">{request.reason}</p>
		</div>
	{/if}

	{#each request.warnings ?? [] as warning (warning)}
		<p
			class="flex items-start gap-2 rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-sm"
		>
			<TriangleAlertIcon class="mt-0.5 size-4 shrink-0 text-warning" />
			<span><span class="font-semibold">Warning:</span> {warning}.</span>
		</p>
	{/each}

	{#if request.lines?.length}
		<pre
			class="max-h-[min(45vh,22rem)] overflow-auto rounded-lg border bg-muted/30 py-2 font-mono text-xs wrap-anywhere whitespace-pre-wrap select-text">{#each request.lines as line, i (i)}<span
					class={cn(
						'block px-3',
						line.kind === 'added' && 'bg-success/10 text-success',
						line.kind === 'removed' && 'bg-danger/10 text-danger'
					)}>{line.text || ' '}</span
				>{/each}</pre>
	{:else if request.detail}
		<pre
			class="max-h-[min(45vh,22rem)] overflow-auto rounded-lg border bg-muted/30 p-3 font-mono text-xs wrap-anywhere whitespace-pre-wrap select-text">{request.detail}</pre>
	{/if}

	{#if request.strong}
		<label class="grid gap-1.5 text-sm" for="approval-confirm">
			<span>
				Type <code class="rounded bg-muted px-1 font-semibold">{request.confirmName}</code> to allow it.
			</span>
			<Input
				id="approval-confirm"
				bind:value={typed}
				autocomplete="off"
				spellcheck="false"
				autofocus
			/>
		</label>
	{/if}

	<Dialog.Footer class="items-center gap-3">
		{#if request.alwaysAllow}
			<p class="mr-auto text-xs text-muted-foreground">
				Always allow: AI apps and the chat stop asking before {request.alwaysAllow.label}, in every
				workspace. Change it in AI - Permissions.
			</p>
		{/if}
		<div class="flex shrink-0 gap-2">
			<Button type="button" variant="ghost" disabled={busy} onclick={() => answer('deny')}>
				Deny
			</Button>
			{#if request.alwaysAllow}
				<Button
					type="button"
					variant="secondary"
					disabled={busy}
					onclick={() => answer('alwaysAllow')}
				>
					Always allow
				</Button>
			{/if}
			<Button
				type="submit"
				variant={request.production ? 'destructive' : 'default'}
				disabled={!allowed || busy}
			>
				Allow once
			</Button>
		</div>
	</Dialog.Footer>
</form>
