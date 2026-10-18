<script lang="ts">
	import { cn } from '#lib/utils.js';

	interface Props {
		/** `columns`: a vertical bar resizing left/right. `rows`: a horizontal bar. */
		direction: 'columns' | 'rows';
		/** Called with the pointer position relative to the parent, in px, while dragging. */
		onresize: (offset: number, parentSize: number) => void;
		onend?: () => void;
	}

	let { direction, onresize, onend }: Props = $props();

	let dragging = $state(false);

	function onpointerdown(e: PointerEvent) {
		const handle = e.currentTarget;
		if (!(handle instanceof HTMLElement)) return;
		const parent = handle.parentElement;
		if (!parent) return;
		e.preventDefault();
		handle.setPointerCapture(e.pointerId);
		dragging = true;
		const rect = parent.getBoundingClientRect();

		const move = (ev: PointerEvent) => {
			if (direction === 'columns') onresize(ev.clientX - rect.left, rect.width);
			else onresize(ev.clientY - rect.top, rect.height);
		};
		const up = () => {
			dragging = false;
			handle.removeEventListener('pointermove', move);
			handle.removeEventListener('pointerup', up);
			onend?.();
		};
		handle.addEventListener('pointermove', move);
		handle.addEventListener('pointerup', up);
	}
</script>

<div
	role="separator"
	aria-orientation={direction === 'columns' ? 'vertical' : 'horizontal'}
	class={cn(
		'group relative z-10 shrink-0 bg-border',
		direction === 'columns' ? 'w-px cursor-col-resize' : 'h-px cursor-row-resize'
	)}
	{onpointerdown}
>
	<!-- A wider invisible hit area, highlighted while hovering or dragging. -->
	<div
		class={cn(
			'absolute transition-colors group-hover:bg-primary/50',
			direction === 'columns' ? '-inset-x-1 inset-y-0' : 'inset-x-0 -inset-y-1',
			dragging && 'bg-primary/60'
		)}
	></div>
</div>
