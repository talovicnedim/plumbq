import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Request } from '#lib/api/index.js';
import { removeRequest, renameRequest } from './actions.js';
import { confirm } from './confirm.svelte.js';
import { prompt } from './prompt.svelte.js';
import { tabs } from './tabs.svelte.js';

const { api, toast } = vi.hoisted(() => ({
	api: {
		requests: { get: vi.fn(), update: vi.fn(), remove: vi.fn() },
		workspace: { tree: vi.fn(async () => []) }
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

const request = (name: string, source: string): Request => ({
	name,
	method: 'GET',
	url: '',
	headers: [],
	body: '',
	assertions: [],
	comments: [],
	trailer: '',
	source
});

async function openTab(id: string, saved: Request) {
	api.requests.get.mockResolvedValueOnce(saved);
	await tabs.open(id);
	const tab = tabs.all.find((t) => t.id === id);
	if (!tab) throw new Error('tab did not open');
	return tab;
}

beforeEach(() => {
	vi.clearAllMocks();
	tabs.all = [];
	tabs.activeId = null;
});

describe('removeRequest', () => {
	it('says the tab has unsaved edits before deleting, and asks only once', async () => {
		const tab = await openTab('list.http', request('List', 'v1'));
		tab.draft.url = 'https://example.test';
		vi.mocked(confirm).mockResolvedValue(true);
		api.requests.remove.mockResolvedValue([{ from: 'list.http', to: null }]);

		await removeRequest('list.http', 'List');

		expect(confirm).toHaveBeenCalledTimes(1);
		expect(vi.mocked(confirm).mock.calls[0][0].description).toMatch(/unsaved/);
		expect(api.requests.remove).toHaveBeenCalledWith('list.http', 'v1');
		expect(tabs.all).toEqual([]);
	});

	it('keeps the request and its tab when the user says no', async () => {
		const tab = await openTab('list.http', request('List', 'v1'));
		tab.draft.url = 'https://example.test';
		vi.mocked(confirm).mockResolvedValue(false);

		await removeRequest('list.http', 'List');

		expect(api.requests.remove).not.toHaveBeenCalled();
		expect(tabs.all).toHaveLength(1);
	});
});

describe('renameRequest', () => {
	it('renames what an edited tab read, so a change on disk still conflicts on its next save', async () => {
		const tab = await openTab('list.http', request('List', 'v1'));
		tab.draft.url = 'https://example.test';
		vi.mocked(prompt).mockResolvedValue('All users');
		api.requests.get.mockResolvedValue(request('List', 'v2 from an agent'));
		api.requests.update.mockResolvedValue(request('All users', 'v3'));

		await renameRequest('list.http', 'List');

		expect(api.requests.update).toHaveBeenCalledWith(
			'list.http',
			expect.objectContaining({ name: 'All users' }),
			'v1'
		);
		expect(tab.saved.source).toBe('v3');
		expect(tab.draft.name).toBe('All users');
		expect(tab.draft.url).toBe('https://example.test');
	});

	it('gives a clean tab the renamed request as on disk', async () => {
		const tab = await openTab('list.http', request('List', 'v1'));
		vi.mocked(prompt).mockResolvedValue('All users');
		api.requests.get.mockResolvedValue(request('List', 'v2 from an agent'));
		api.requests.update.mockResolvedValue(request('All users', 'v3'));

		await renameRequest('list.http', 'List');

		expect(api.requests.update).toHaveBeenCalledWith(
			'list.http',
			expect.objectContaining({ name: 'All users' }),
			'v2 from an agent'
		);
		expect(tab.saved).toEqual(request('All users', 'v3'));
		expect(tabs.isDirty(tab)).toBe(false);
	});
});
