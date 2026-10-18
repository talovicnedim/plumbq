<script lang="ts">
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import { goto } from '$app/navigation';
	import { settingsSections } from '#lib/components/settings/sections.js';
	import { Button } from '#lib/components/ui/button/index.js';

	function back(e: KeyboardEvent) {
		if (e.key === 'Escape' && !e.defaultPrevented) void goto('/');
	}
</script>

<svelte:window onkeydown={back} />

<main class="h-full overflow-y-auto">
	<div data-tauri-drag-region class="h-9"></div>
	<div class="mx-auto max-w-2xl px-8 pb-12">
		<Button href="/" variant="ghost" size="sm" class="mb-4 -ml-2">
			<ChevronLeftIcon /> Back
		</Button>
		<h1 class="mb-8 text-xl font-semibold">Settings</h1>

		{#each settingsSections as section (section.id)}
			<section class="mb-8" aria-labelledby={`settings-${section.id}`}>
				<h2
					id={`settings-${section.id}`}
					class="mb-3 text-xs font-semibold tracking-wider text-muted-foreground uppercase"
				>
					{section.title}
				</h2>
				<div class="rounded-lg border px-4 py-1.5">
					<section.component />
				</div>
			</section>
		{/each}
	</div>
</main>
