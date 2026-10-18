import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Request, WorkspaceSummary } from '#lib/api/index.js';
import { assistant } from './assistant.svelte.js';
import { confirm } from './confirm.svelte.js';
import { prompt } from './prompt.svelte.js';
import { tabs } from './tabs.svelte.js';
import { workspace } from './workspace.svelte.js';

const { api, toast } = vi.hoisted(() => ({
	api: {
		requests: { get: vi.fn() },
		workspace: {
			current: vi.fn(),
			open: vi.fn(),
			rename: vi.fn(),
			recent: vi.fn(async (): Promise<WorkspaceSummary[]> => []),
			tree: vi.fn(async () => [])
		},
		assistant: {
			stop: vi.fn(async () => undefined),
			chats: { list: vi.fn(async () => []) }
		}
	},
	toast: { warning: vi.fn(), error: vi.fn(), success: vi.fn() }
}));
vi.mock('#lib/api/index.js', () => ({
	api,
	onAssistantEvent: () => () => {},
	toApiError: (err: unknown) => err
}));
vi.mock('svelte-sonner', () => ({ toast }));
vi.mock('./confirm.svelte.js', () => ({ confirm: vi.fn() }));
vi.mock('./prompt.svelte.js', () => ({ prompt: vi.fn() }));

const request: Request = {
	name: 'List',
	method: 'GET',
	url: '',
	headers: [],
	body: '',
	assertions: [],
	comments: [],
	trailer: '',
	source: 'v1'
};

async function openEditedTab() {
	api.requests.get.mockResolvedValueOnce(request);
	await tabs.open('list.http');
	tabs.all[0].draft.url = 'https://example.test';
}

beforeEach(() => {
	vi.clearAllMocks();
	tabs.all = [];
	tabs.activeId = null;
	workspace.root = '/work/a';
});

describe('switching workspace', () => {
	it('keeps the tabs when the other workspace fails to open', async () => {
		await openEditedTab();
		vi.mocked(confirm).mockResolvedValue(true);
		api.workspace.open.mockRejectedValue({ kind: 'notFound', message: 'gone' });

		expect(await workspace.open('/work/b')).toBe(false);

		expect(tabs.all).toHaveLength(1);
		expect(tabs.all[0].draft.url).toBe('https://example.test');
		expect(workspace.root).toBe('/work/a');
		expect(toast.error).toHaveBeenCalledWith('gone');
	});

	it('asks about unsaved edits before opening, and stays when told no', async () => {
		await openEditedTab();
		vi.mocked(confirm).mockResolvedValue(false);

		expect(await workspace.open('/work/b')).toBe(false);

		expect(api.workspace.open).not.toHaveBeenCalled();
		expect(tabs.all).toHaveLength(1);
	});

	it('closes the tabs and starts a new chat once the other workspace is open', async () => {
		await openEditedTab();
		vi.mocked(confirm).mockResolvedValue(true);
		api.workspace.open.mockResolvedValue({ root: '/work/b', name: 'Billing API' });
		const oldChat = assistant.chatId;
		assistant.items = [{ kind: 'user', text: 'about workspace a' }];
		assistant.pending = 'explain this';

		expect(await workspace.open('/work/b')).toBe(true);

		expect(workspace.root).toBe('/work/b');
		expect(workspace.name).toBe('Billing API');
		expect(tabs.all).toEqual([]);
		expect(confirm).toHaveBeenCalledTimes(1);
		expect(assistant.chatId).not.toBe(oldChat);
		expect(assistant.items).toEqual([]);
		expect(assistant.pending).toBeUndefined();
	});
});

describe('loading at start', () => {
	it('still shows the Welcome screen, with the problem, when the recent list is unreadable', async () => {
		workspace.root = null;
		workspace.loaded = false;
		api.workspace.current.mockResolvedValue(null);
		api.workspace.recent.mockRejectedValueOnce({
			kind: 'invalid',
			message: 'failed to read the list of recent workspaces /data/recent.json'
		});

		await workspace.load();

		expect(workspace.loaded).toBe(true);
		expect(workspace.problem).toMatch(/recent\.json/);
	});
});

describe('renaming the workspace', () => {
	it('shows the new name and refreshes the recent list', async () => {
		workspace.name = 'my-api';
		vi.mocked(prompt).mockResolvedValue('Jellyreach API');
		api.workspace.rename.mockResolvedValue({ root: '/work/a', name: 'Jellyreach API' });
		api.workspace.recent.mockResolvedValueOnce([{ root: '/work/a', name: 'Jellyreach API' }]);

		await workspace.rename();

		expect(prompt).toHaveBeenCalledWith(expect.objectContaining({ initial: 'my-api' }));
		expect(api.workspace.rename).toHaveBeenCalledWith('Jellyreach API');
		expect(workspace.name).toBe('Jellyreach API');
		expect(workspace.recent).toEqual([{ root: '/work/a', name: 'Jellyreach API' }]);
	});

	it('keeps the name and says why when the rename is refused', async () => {
		workspace.name = 'my-api';
		vi.mocked(prompt).mockResolvedValue('two\nlines');
		const message = 'a workspace name is one line of up to 80 characters';
		api.workspace.rename.mockRejectedValue({ kind: 'invalid', message });

		await workspace.rename();

		expect(workspace.name).toBe('my-api');
		expect(toast.error).toHaveBeenCalledWith(message);
	});

	it('changes nothing when cancelled', async () => {
		vi.mocked(prompt).mockResolvedValue(null);

		await workspace.rename();

		expect(api.workspace.rename).not.toHaveBeenCalled();
	});
});
