import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Request } from '#lib/api/index.js';
import { tabs } from './tabs.svelte.js';

const { api, toast } = vi.hoisted(() => ({
	api: { requests: { get: vi.fn(), update: vi.fn() } },
	toast: { warning: vi.fn(), error: vi.fn(), success: vi.fn() }
}));
vi.mock('#lib/api/index.js', () => ({ api, toApiError: (err: unknown) => err }));
vi.mock('svelte-sonner', () => ({ toast }));

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

beforeEach(() => {
	vi.clearAllMocks();
	tabs.all = [];
	tabs.activeId = null;
});

describe('tabs', () => {
	it('shows one conflict toast per tab however often saving fails', async () => {
		api.requests.get.mockResolvedValue(request('List', 'v1'));
		api.requests.update.mockRejectedValue({ kind: 'conflict', message: 'changed' });
		await tabs.open('list.http');
		const tab = tabs.all[0];
		tab.draft.url = 'https://example.test';

		await tabs.save(tab);
		await tabs.save(tab);

		const ids = toast.warning.mock.calls.map(([, options]) => options.id);
		expect(ids).toHaveLength(2);
		expect(ids[0]).toBeDefined();
		expect(ids[1]).toBe(ids[0]);
	});
});
