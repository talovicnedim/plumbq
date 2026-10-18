import type { Component } from 'svelte';
import CookieIcon from '@lucide/svelte/icons/cookie';
import FileTextIcon from '@lucide/svelte/icons/file-text';
import FolderPlusIcon from '@lucide/svelte/icons/folder-plus';
import HistoryIcon from '@lucide/svelte/icons/history';
import ImportIcon from '@lucide/svelte/icons/import';
import KeyboardIcon from '@lucide/svelte/icons/keyboard';
import LayoutIcon from '@lucide/svelte/icons/layout-panel-left';
import MoonIcon from '@lucide/svelte/icons/moon';
import PencilIcon from '@lucide/svelte/icons/pencil';
import PlayIcon from '@lucide/svelte/icons/play';
import SettingsIcon from '@lucide/svelte/icons/settings-2';
import UploadIcon from '@lucide/svelte/icons/upload';
import { toast } from 'svelte-sonner';
import { api, toApiError } from '#lib/api/index.js';
import {
	quickEntries,
	type PaletteEntry,
	type PaletteHooks
} from '#lib/components/palette/entries.js';
import { importCurl, newFolder } from '#lib/state/actions.js';
import { confirm } from '#lib/state/confirm.svelte.js';
import { environments } from '#lib/state/environments.svelte.js';
import { importer } from '#lib/state/importer.svelte.js';
import { layout } from '#lib/state/layout.svelte.js';
import { overlays } from '#lib/state/overlays.svelte.js';
import { exportDocs, exportPostman, runner } from '#lib/state/runner.svelte.js';
import { settings } from '#lib/state/settings.svelte.js';
import { workspace } from '#lib/state/workspace.svelte.js';
import { workspaceSettings } from '#lib/state/workspace-settings.svelte.js';
import type { Theme } from '#lib/utils/settings.js';
import { labelFor } from '#lib/utils/shortcuts.js';

/** Every command, for `>` and for searching everything. */
export function commandEntries(hooks: PaletteHooks): PaletteEntry[] {
	const command = (
		id: string,
		icon: Component,
		title: string,
		run: () => unknown,
		more: Partial<PaletteEntry> = {}
	): PaletteEntry => ({ id: `cmd:${id}`, kind: 'Command', icon, title, run, ...more });
	return [
		...quickEntries(hooks),
		command('recent', HistoryIcon, 'Recent requests', () => (overlays.recent = true), {
			shortcut: labelFor('recent')
		}),
		command('new-folder', FolderPlusIcon, 'New folder', () => newFolder('')),
		command('new-workspace', FolderPlusIcon, 'New workspace', () => workspace.create()),
		command('rename-workspace', PencilIcon, 'Rename workspace', () => workspace.rename(), {
			subtitle: workspace.name
		}),
		command('import-curl', ImportIcon, 'Import cURL', () => importCurl('')),
		command('import-file', ImportIcon, 'Import a file', () => importer.show('', 'file'), {
			subtitle: 'Postman, OpenAPI, Swagger or Insomnia'
		}),
		command('run-all', PlayIcon, 'Run all requests', () => runner.show(''), {
			subtitle: 'With assertions, data file, tags'
		}),
		command('export-postman', UploadIcon, 'Export to Postman collection', () => exportPostman('')),
		command('export-docs', FileTextIcon, 'Generate HTML docs', exportDocs),
		command(
			'workspace-settings',
			SettingsIcon,
			'Workspace settings',
			() => (workspaceSettings.open = true),
			{ subtitle: 'Timeouts, proxy, certificates' }
		),
		command('environments', SettingsIcon, 'Manage environments', hooks.manageEnvironments),
		command('layout', LayoutIcon, 'Toggle side-by-side response', () => layout.toggleOrientation()),
		command(
			'cookies',
			CookieIcon,
			`Clear cookies for ${environments.selected ?? 'no environment'}`,
			clearCookies
		),
		command('shortcuts', KeyboardIcon, 'Keyboard shortcuts', () => (overlays.shortcuts = true), {
			shortcut: labelFor('shortcuts')
		}),
		...themeEntries()
	];
}

function themeEntries(): PaletteEntry[] {
	const themes: [Theme, string][] = [
		['light', 'Light theme'],
		['dark', 'Dark theme'],
		['system', 'System theme']
	];
	return themes.map(([theme, title]) => ({
		id: `cmd:theme-${theme}`,
		kind: 'Command',
		icon: MoonIcon,
		title,
		meta: settings.values.theme === theme ? 'current' : undefined,
		run: () => settings.update({ theme })
	}));
}

async function clearCookies() {
	const env = environments.selected;
	const ok = await confirm({
		title: `Clear cookies for ${env ?? 'no environment'}?`,
		description: 'Every stored cookie of this environment is deleted, e.g. to log out.',
		confirmLabel: 'Clear cookies',
		destructive: true
	});
	if (!ok) return;
	try {
		await api.cookies.clear(env);
		toast.success('Cookies cleared');
	} catch (err) {
		toast.error(toApiError(err).message);
	}
}
