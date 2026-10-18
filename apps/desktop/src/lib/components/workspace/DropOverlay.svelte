<script lang="ts">
	import FileUpIcon from '@lucide/svelte/icons/file-up';
	import { onFileDrag } from '#lib/api/index.js';
	import { importer } from '#lib/state/importer.svelte.js';

	let dragging = $state(false);

	// Files or a Bruno collection folder dropped anywhere on the window are imported into the
	// workspace root. Windows turns native drops off (tauri.windows.conf.json) so dragging
	// requests and tabs works; there, importing goes through the Import dialog.
	$effect(() =>
		onFileDrag((drag) => {
			// Only drags that carry files; dragging a request or tab inside the app has none.
			if (drag.kind === 'enter') dragging = drag.paths.length > 0;
			if (drag.kind === 'leave') dragging = false;
			if (drag.kind === 'drop') {
				dragging = false;
				const [path] = drag.paths;
				if (path) {
					importer.folder = '';
					void importer.file(path);
				}
			}
		})
	);
</script>

{#if dragging}
	<div
		class="pointer-events-none fixed inset-3 z-50 flex flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-primary bg-background/85 text-sm backdrop-blur-sm"
	>
		<FileUpIcon class="size-8 text-primary" />
		<p class="font-medium">Drop to import</p>
		<p class="text-xs text-muted-foreground">
			Postman, OpenAPI, Swagger, Insomnia, HAR or a Bruno folder
		</p>
	</div>
{/if}
