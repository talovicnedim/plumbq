<script lang="ts">
	import type { VariableHint } from '#lib/api/index.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { cn } from '#lib/utils.js';
	import { segments } from '#lib/utils/variables.js';
	import VariableCard from './VariableCard.svelte';

	interface Props {
		value: string;
		placeholder?: string;
		/** How each `{{variable}}` resolves, from core. */
		variables: VariableHint[];
		/** The box around the input: where it sits, its border and focus color. */
		class?: string;
		/** Padding and font size of the input and of its colored copy. They must match. */
		textClass?: string;
	}

	let {
		value = $bindable(),
		placeholder,
		variables,
		class: className,
		textClass = 'px-3 text-[13px]'
	}: Props = $props();

	interface Hovered {
		hint: VariableHint;
		left: number;
		top: number;
	}

	/** Like the body editor's hover, so a mouse passing over the URL doesn't flash cards. */
	const HOVER_MS = 300;
	const CARD_WIDTH = 320;

	let scroll = $state(0);
	let overlay = $state<HTMLElement | null>(null);
	let hovered = $state<Hovered | null>(null);
	let timer: ReturnType<typeof setTimeout> | undefined;

	const parts = $derived(segments(value));
	const hints = $derived(new Map(variables.map((v) => [v.name, v])));

	// The input lies on top and takes the mouse, so the variable under it is found by position.
	function onmousemove(e: MouseEvent) {
		const next = e.buttons === 0 ? variableAt(e.clientX) : null;
		if (next && hovered && next.left === hovered.left) return;
		clearTimeout(timer);
		if (!next) {
			hovered = null;
			return;
		}
		timer = setTimeout(() => (hovered = next), hovered ? 0 : HOVER_MS);
	}

	function variableAt(x: number): Hovered | null {
		for (const span of overlay?.querySelectorAll<HTMLElement>('[data-variable]') ?? []) {
			const rect = span.getBoundingClientRect();
			if (x < rect.left || x > rect.right) continue;
			const hint = hints.get(span.dataset.variable ?? '');
			if (!hint) return null;
			const left = Math.max(8, Math.min(rect.left, window.innerWidth - CARD_WIDTH - 8));
			return { hint, left, top: rect.bottom + 6 };
		}
		return null;
	}

	function hide() {
		clearTimeout(timer);
		hovered = null;
	}

	function follow(e: Event & { currentTarget: HTMLInputElement }) {
		scroll = e.currentTarget.scrollLeft;
	}
</script>

<!-- A transparent input over a highlighted copy of its text: native editing, colored variables. -->
<div class={cn('relative min-w-0 overflow-hidden', className)}>
	<div
		bind:this={overlay}
		aria-hidden="true"
		class={cn(
			'pointer-events-none absolute inset-0 flex items-center font-mono whitespace-pre',
			textClass
		)}
	>
		<span style={`transform: translateX(${-scroll}px)`}>
			{#each parts as part, i (i)}
				{#if part.variable === null}
					<span>{part.text}</span>
				{:else}
					<span
						data-variable={part.variable}
						class={cn(
							'rounded-sm',
							hints.get(part.variable)?.kind === 'missing'
								? 'text-destructive underline decoration-destructive decoration-wavy underline-offset-[3px]'
								: 'bg-primary/15 text-primary'
						)}>{part.text}</span
					>
				{/if}
			{/each}
		</span>
	</div>
	<input
		class={cn(
			'relative h-full w-full bg-transparent font-mono text-transparent caret-foreground outline-none selection:bg-primary/25 selection:text-transparent placeholder:text-muted-foreground',
			textClass
		)}
		bind:value
		{placeholder}
		spellcheck="false"
		autocomplete="off"
		onscroll={follow}
		oninput={(e) => {
			hide();
			follow(e);
		}}
		onkeyup={follow}
		onclick={follow}
		{onmousemove}
		onmouseleave={hide}
		onmousedown={hide}
	/>
	{#if hovered}
		<div
			class="pointer-events-none fixed z-50"
			style={`left: ${hovered.left}px; top: ${hovered.top}px`}
		>
			<VariableCard hint={hovered.hint} environment={environments.selected} />
		</div>
	{/if}
</div>
