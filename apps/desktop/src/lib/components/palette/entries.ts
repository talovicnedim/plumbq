import type { Component } from 'svelte';
import { goto } from '$app/navigation';
import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
import LayersIcon from '@lucide/svelte/icons/layers';
import PlusIcon from '@lucide/svelte/icons/plus';
import SettingsIcon from '@lucide/svelte/icons/settings-2';
import SparklesIcon from '@lucide/svelte/icons/sparkles';
import { newRequest } from '#lib/state/actions.js';
import { environments } from '#lib/state/environments.svelte.js';
import { recentRequests } from '#lib/state/recent-requests.svelte.js';
import { tabs } from '#lib/state/tabs.svelte.js';
import { workspace } from '#lib/state/workspace.svelte.js';
import { ENVIRONMENT_BG, ENVIRONMENT_LABEL } from '#lib/utils/environments.js';
import { recentFirst, type Searchable } from '#lib/utils/palette.js';
import { flatten, type RequestEntry } from '#lib/utils/search.js';
import { labelFor } from '#lib/utils/shortcuts.js';

export interface PaletteEntry extends Searchable {
	id: string;
	/** What kind of thing it is, shown on the right while searching everything. */
	kind: 'Request' | 'Command' | 'Environment' | 'Workspace';
	icon?: Component;
	/** Request rows show their method where other rows show an icon. */
	method?: string;
	/** A colored dot instead of an icon, as a full `bg-*` class. */
	dot?: string;
	shortcut?: string;
	/** Quiet text on the right, e.g. "current". */
	meta?: string;
	/** Status of the last send. */
	status?: number;
	/** Choosing it types this prefix into the search instead of running something. */
	prefix?: string;
	run?: () => unknown;
}

/** What the palette needs from the page around it. */
export interface PaletteHooks {
	manageEnvironments: () => void;
	connectAi: () => void;
}

/** Every request, the recently used ones first so they win ties. */
export function requestEntries(): PaletteEntry[] {
	const status = new Map(recentRequests.list.map((r) => [r.id, r.status]));
	const requests = recentFirst(
		flatten(workspace.tree.data ?? []),
		recentRequests.list.map((r) => r.id)
	);
	return requests.map((request) => requestEntry(request, status.get(request.id)));
}

/** Up to five requests sent lately that still exist. */
export function recentlySent(requests: PaletteEntry[]): PaletteEntry[] {
	const byId = new Map(requests.map((entry) => [entry.id, entry]));
	return recentRequests.list
		.filter((r) => r.sentAt !== undefined)
		.flatMap((r) => byId.get(`request:${r.id}`) ?? [])
		.slice(0, 5);
}

function requestEntry(request: RequestEntry, status: number | undefined): PaletteEntry {
	return {
		id: `request:${request.id}`,
		kind: 'Request',
		method: request.method,
		title: request.name,
		subtitle: request.folder,
		keywords: `${request.method} ${request.url} ${request.id}`,
		tags: request.tags,
		status,
		run: () => tabs.open(request.id)
	};
}

/** The few actions the empty palette offers. */
export function quickEntries(hooks: PaletteHooks): PaletteEntry[] {
	const command = (
		id: string,
		icon: Component,
		title: string,
		run: () => unknown,
		shortcut?: string
	): PaletteEntry => ({ id: `quick:${id}`, kind: 'Command', icon, title, run, shortcut });
	return [
		command('new-request', PlusIcon, 'New request', () => newRequest(''), labelFor('newRequest')),
		{
			id: 'quick:environment',
			kind: 'Command',
			icon: LayersIcon,
			title: 'Switch environment',
			subtitle: environments.selected ?? 'No environment',
			shortcut: '@',
			prefix: '@'
		},
		command(
			'open-workspace',
			FolderOpenIcon,
			'Open workspace',
			() => workspace.choose(),
			labelFor('openWorkspace')
		),
		{
			...command('ai', SparklesIcon, 'AI settings', hooks.connectAi, labelFor('aiMcp')),
			subtitle: 'Chat model, connect Claude, ChatGPT, Cursor, VS Code'
		},
		command('settings', SettingsIcon, 'Settings', () => goto('/settings'), labelFor('settings'))
	];
}

export function environmentEntries(): PaletteEntry[] {
	const current = (name: string | null) => (environments.selected === name ? 'current' : undefined);
	return [
		{
			id: 'environment:',
			kind: 'Environment',
			title: 'No environment',
			dot: 'bg-muted-foreground/40',
			meta: current(null),
			run: () => environments.select(null)
		},
		...(environments.list.data ?? []).map((env): PaletteEntry => ({
			id: `environment:${env.name}`,
			kind: 'Environment',
			title: `Use ${env.name}`,
			subtitle: ENVIRONMENT_LABEL[env.tag],
			dot: ENVIRONMENT_BG[env.tag],
			meta: current(env.name),
			run: () => environments.select(env.name)
		}))
	];
}

/** Recently opened workspaces other than this one. */
export function workspaceEntries(): PaletteEntry[] {
	return workspace.recent
		.filter((recent) => recent.root !== workspace.root)
		.map((recent) => ({
			id: `workspace:${recent.root}`,
			kind: 'Workspace',
			icon: FolderOpenIcon,
			title: `Switch to ${recent.name}`,
			subtitle: recent.root,
			run: () => workspace.open(recent.root)
		}));
}
