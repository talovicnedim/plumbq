<script lang="ts">
	import Columns2Icon from '@lucide/svelte/icons/columns-2';
	import Rows2Icon from '@lucide/svelte/icons/rows-2';
	import SearchIcon from '@lucide/svelte/icons/search';
	import MessageSquareIcon from '@lucide/svelte/icons/message-square';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import ChatPanel from '#lib/components/ai/ChatPanel.svelte';
	import { ai } from '#lib/state/ai.svelte.js';
	import { assistant } from '#lib/state/assistant.svelte.js';
	import Splitter from '#lib/components/common/Splitter.svelte';
	import EnvironmentPicker from '#lib/components/environments/EnvironmentPicker.svelte';
	import EnvironmentsDialog from '#lib/components/environments/EnvironmentsDialog.svelte';
	import CommandPalette from '#lib/components/palette/CommandPalette.svelte';
	import RecentSwitcher from '#lib/components/palette/RecentSwitcher.svelte';
	import StatusBar from '#lib/components/status/StatusBar.svelte';
	import EmptyEditor from '#lib/components/workspace/EmptyEditor.svelte';
	import RequestEditor from '#lib/components/request/RequestEditor.svelte';
	import RequestTabBar from '#lib/components/request/RequestTabBar.svelte';
	import ResponseView from '#lib/components/response/ResponseView.svelte';
	import DropOverlay from '#lib/components/workspace/DropOverlay.svelte';
	import ImportDialog from '#lib/components/workspace/ImportDialog.svelte';
	import RunnerDialog from '#lib/components/runner/RunnerDialog.svelte';
	import Sidebar from '#lib/components/workspace/Sidebar.svelte';
	import Welcome from '#lib/components/workspace/Welcome.svelte';
	import WorkspaceSettingsDialog from '#lib/components/workspace/WorkspaceSettingsDialog.svelte';
	import { environments } from '#lib/state/environments.svelte.js';
	import { layout } from '#lib/state/layout.svelte.js';
	import { overlays } from '#lib/state/overlays.svelte.js';
	import { recentRequests } from '#lib/state/recent-requests.svelte.js';
	import { settings } from '#lib/state/settings.svelte.js';
	import { palette } from '#lib/state/palette.svelte.js';
	import { tabs } from '#lib/state/tabs.svelte.js';
	import { workspace } from '#lib/state/workspace.svelte.js';
	import { cn } from '#lib/utils.js';
	import { dialogOpen, matchShortcut, labelFor } from '#lib/utils/shortcuts.js';

	let manageEnvironments = $state(false);
	let switcherOpen = $state(false);

	const columns = $derived(layout.orientation === 'columns');

	// A different workspace has its own environments (its tabs were closed before switching).
	$effect(() => {
		const root = workspace.root;
		recentRequests.load(root);
		if (!root) return;
		void environments.load(root);
	});

	// Auto-save (Settings → Editor): the active tab saves a second after the last edit. Reading
	// the draft as JSON tracks every field, so each keystroke restarts the wait.
	$effect(() => {
		const tab = tabs.active;
		if (!settings.values.autoSave || !tab || tab.sending) return;
		const draft = JSON.stringify(tab.draft);
		if (draft === JSON.stringify(tab.saved)) return;
		const timer = setTimeout(() => void tabs.save(tab), 1000);
		return () => clearTimeout(timer);
	});

	function onkeydown(e: KeyboardEvent) {
		const match = matchShortcut(e);
		if (!match || !workspace.root) return;
		// The palette shortcut also closes the palette; others wait until dialogs are closed.
		if (dialogOpen() && !(match.id === 'palette' && palette.open)) return;
		if (match.id === 'palette') palette.open = !palette.open;
		else if (match.id === 'recent') overlays.recent = true;
		else if (match.id === 'switchWorkspace') switcherOpen = true;
		else return;
		e.preventDefault();
	}
</script>

<svelte:window {onkeydown} />

{#if workspace.loaded && !workspace.root}
	<Welcome />
{:else if workspace.root}
	<div class="flex h-full flex-col">
		<div class="flex min-h-0 flex-1">
			<div class="shrink-0" style={`width: ${layout.sidebarWidth}px`}>
				<Sidebar bind:switcherOpen />
			</div>
			<Splitter
				direction="columns"
				onresize={(x) => (layout.sidebarWidth = Math.min(480, Math.max(200, x)))}
				onend={() => layout.save()}
			/>

			<main class="flex min-w-0 flex-1 flex-col">
				<header data-tauri-drag-region class="flex h-11 shrink-0 items-center gap-2 border-b px-3">
					<button
						type="button"
						class="flex h-7 w-72 items-center gap-2 rounded-md border bg-muted/40 px-2.5 text-xs text-muted-foreground hover:bg-muted"
						onclick={() => (palette.open = true)}
					>
						<SearchIcon class="size-3.5" /> Search requests and commands
						<kbd class="ml-auto text-[10px]">{labelFor('palette')}</kbd>
					</button>
					<div data-tauri-drag-region class="flex-1 self-stretch"></div>
					<button
						type="button"
						class="flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-xs hover:bg-muted"
						title={`AI: chat model, AI apps, permissions, activity (${labelFor('aiMcp')})`}
						onclick={() => ai.open(ai.pending.length > 0 ? 'activity' : 'chat')}
					>
						<SparklesIcon class="size-3.5 text-primary" /> AI
						{#if ai.pending.length > 0}
							<span class="rounded-full bg-env-staging px-1.5 text-[10px] font-semibold text-white">
								{ai.pending.length}
							</span>
						{:else if ai.running}
							<span class="size-1.5 rounded-full bg-env-local" title="MCP server running"></span>
						{/if}
					</button>
					{#if !ai.off}
						<button
							type="button"
							class={cn(
								'flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-xs hover:bg-muted',
								assistant.open && 'border-primary/50 bg-primary/10'
							)}
							title={`Chat (${labelFor('assistant')})`}
							aria-pressed={assistant.open}
							onclick={() => assistant.toggle()}
						>
							<MessageSquareIcon class="size-3.5 text-primary" /> Chat
						</button>
					{/if}
					<button
						type="button"
						class="rounded-md p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground"
						title={columns ? 'Show response below' : 'Show response on the side'}
						onclick={() => layout.toggleOrientation()}
					>
						{#if columns}<Rows2Icon class="size-4" />{:else}<Columns2Icon class="size-4" />{/if}
					</button>
					<EnvironmentPicker onmanage={() => (manageEnvironments = true)} />
				</header>

				{#if tabs.all.length > 0}
					<RequestTabBar />
				{/if}

				{#if tabs.active}
					{#key tabs.active.id}
						<div class={cn('flex min-h-0 flex-1', columns ? 'flex-row' : 'flex-col')}>
							<div class="min-h-0 min-w-0" style={`flex: ${layout.split} 1 0`}>
								<RequestEditor
									tab={tabs.active}
									onmanageEnvironments={() => (manageEnvironments = true)}
								/>
							</div>
							<Splitter
								direction={columns ? 'columns' : 'rows'}
								onresize={(offset, size) =>
									(layout.split = Math.min(0.8, Math.max(0.2, offset / size)))}
								onend={() => layout.save()}
							/>
							<div class="min-h-0 min-w-0 bg-muted/20" style={`flex: ${1 - layout.split} 1 0`}>
								<ResponseView tab={tabs.active} />
							</div>
						</div>
					{/key}
				{:else}
					<EmptyEditor />
				{/if}
			</main>
			{#if assistant.open && !ai.off}
				<Splitter
					direction="columns"
					onresize={(x, width) => (layout.assistantWidth = Math.min(640, Math.max(300, width - x)))}
					onend={() => layout.save()}
				/>
				<div class="shrink-0" style={`width: ${layout.assistantWidth}px`}>
					<ChatPanel />
				</div>
			{/if}
		</div>
		<StatusBar />
	</div>
	<EnvironmentsDialog bind:open={manageEnvironments} />
	<CommandPalette
		onmanageEnvironments={() => (manageEnvironments = true)}
		onconnectAi={() => ai.open()}
	/>
	<ImportDialog />
	<WorkspaceSettingsDialog />
	<RunnerDialog />
	<RecentSwitcher />
	<DropOverlay />
{/if}
