<script lang="ts">
	import './layout.css';
	import { ModeWatcher, setMode } from 'mode-watcher';
	import { untrack } from 'svelte';
	import { onCloseRequested, onMenu, onQuitRequested, onWorkspaceChanged } from '#lib/api/index.js';
	import { goto } from '$app/navigation';
	import ConfirmDialog from '#lib/components/common/ConfirmDialog.svelte';
	import ShortcutsDialog from '#lib/components/help/ShortcutsDialog.svelte';
	import AiRoot from '#lib/components/ai/AiRoot.svelte';
	import PromptDialog from '#lib/components/common/PromptDialog.svelte';
	import PromptValuesDialog from '#lib/components/common/PromptValuesDialog.svelte';
	import { Toaster } from '#lib/components/ui/sonner/index.js';
	import { newRequest } from '#lib/state/actions.js';
	import { ai } from '#lib/state/ai.svelte.js';
	import { overlays } from '#lib/state/overlays.svelte.js';
	import { settings } from '#lib/state/settings.svelte.js';
	import { environments } from '#lib/state/environments.svelte.js';
	import { tabs } from '#lib/state/tabs.svelte.js';
	import { checkForUpdates } from '#lib/state/updates.js';
	import { quitApp } from '#lib/state/quit.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { dialogOpen, matchShortcut, type ShortcutId } from '#lib/utils/shortcuts.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	$effect(() => {
		if (!workspace.loaded) void workspace.load();
	});

	// Once per launch; only release builds with the updater configured find anything.
	$effect(() => {
		if (untrack(() => settings.values.checkForUpdates)) void checkForUpdates();
	});

	// Density and editor preferences apply app-wide through attributes and CSS variables.
	$effect(() => {
		const root = document.documentElement;
		const theme = settings.values.theme;
		// `setMode` reads mode-watcher's own state; tracking that here makes the effect loop.
		untrack(() => setMode(theme));
		root.dataset.density = settings.values.density;
		root.style.setProperty('--editor-font-size', `${settings.values.editorFontSize}px`);
	});

	// Files changed on disk (git, an editor, an AI agent): reload, debounced because one save can
	// fire several events.
	$effect(() => {
		let timer: ReturnType<typeof setTimeout> | undefined;
		const stop = onWorkspaceChanged(() => {
			clearTimeout(timer);
			timer = setTimeout(() => {
				void workspace.tree.refresh();
				void workspace.refreshName();
				void environments.list.refresh();
				void environments.defaults.refresh();
				void tabs.reload();
			}, 150);
		});
		return () => {
			clearTimeout(timer);
			stop();
		};
	});

	$effect(() =>
		onMenu((action) => {
			if (action === 'open-workspace') void workspace.choose();
			if (action === 'new-request' && workspace.root) void newRequest('');
			if (action === 'close-tab' && tabs.activeId) void tabs.close(tabs.activeId);
			if (action === 'settings') void goto('/settings');
		})
	);

	// Quitting (⌘Q, the menu) and closing the window ask first when tabs have unsaved changes.
	$effect(() => onQuitRequested(() => void quitApp()));
	$effect(() => onCloseRequested(() => void quitApp()));

	// App shortcuts come from one registry (`#lib/utils/shortcuts.js`), also shown by ⌘/. Menu
	// shortcuts on macOS (⌘N, ⌘O, ⌘W, ⌘,) arrive through `onMenu` above instead.
	function onkeydown(e: KeyboardEvent) {
		const match = matchShortcut(e);
		if (!match) return;
		const sending = tabs.active?.sending ? tabs.active : undefined;
		if (match.id === 'cancel') {
			// Esc only when no dialog took it; ⌘. always.
			const plainEsc = e.key === 'Escape';
			if (!sending || (plainEsc && (e.defaultPrevented || dialogOpen()))) return;
			e.preventDefault();
			void tabs.cancel(sending);
			return;
		}
		// Shortcuts act on the request behind a dialog otherwise (⌘↵ in the Import dialog…).
		if (dialogOpen()) return;
		const tab = tabs.active;
		const handled = run(match.id, match.index, tab);
		if (handled) e.preventDefault();
	}

	/** Runs an app-wide shortcut; `false` when it doesn't apply right now. */
	function run(id: ShortcutId, index: number | undefined, tab: typeof tabs.active): boolean {
		switch (id) {
			case 'send':
				if (!tab) return false;
				void tabs.send(tab);
				return true;
			case 'save':
				if (!tab) return false;
				void tabs.save(tab);
				return true;
			case 'openWorkspace':
				void workspace.choose();
				return true;
			case 'selectTab': {
				const target = index === undefined ? undefined : tabs.all[index];
				if (!target) return false;
				tabs.activeId = target.id;
				return true;
			}
			case 'newRequest':
				if (!workspace.root) return false;
				void newRequest('');
				return true;
			case 'closeAllTabs':
				if (tabs.all.length === 0) return false;
				void tabs.closeEverything();
				return true;
			case 'closeTab':
				if (!tabs.activeId) return false;
				void tabs.close(tabs.activeId);
				return true;
			case 'settings':
				void goto('/settings');
				return true;
			case 'shortcuts':
				overlays.shortcuts = !overlays.shortcuts;
				return true;
			case 'aiMcp':
				ai.open();
				return true;
			case 'assistant':
				if (!workspace.root || ai.off) return false;
				assistant.toggle();
				return true;
			default:
				// palette, recent and switchWorkspace belong to the workspace page.
				return false;
		}
	}
</script>

<svelte:window {onkeydown} />
<!-- No head script: it would violate the CSP and does nothing in a client-only app. -->
<ModeWatcher disableHeadScriptInjection />
<Toaster position="bottom-right" />
<ConfirmDialog />
<PromptDialog />
<PromptValuesDialog />
<ShortcutsDialog />
<AiRoot />

<div class="h-full">{@render children()}</div>
