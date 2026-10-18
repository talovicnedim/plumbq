import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Environment } from '#lib/api/index.js';
import { confirm } from './confirm.svelte.js';
import { EnvironmentEditor } from './environment-editor.svelte.js';
import { environments } from './environments.svelte.js';

const { api } = vi.hoisted(() => ({
	api: {
		environments: { create: vi.fn(), list: vi.fn(), warnings: vi.fn(async () => []) },
		folders: { getSettings: vi.fn() }
	}
}));
vi.mock('#lib/api/index.js', () => ({ api, toApiError: (err: unknown) => err }));
vi.mock('./confirm.svelte.js', () => ({ confirm: vi.fn() }));

const env = (name: string, variables: Record<string, string> = {}): Environment => ({
	name,
	tag: 'local',
	variables,
	secrets: [],
	auth: []
});

let editor: EnvironmentEditor;

beforeEach(() => {
	vi.clearAllMocks();
	editor = new EnvironmentEditor();
});

describe('EnvironmentEditor', () => {
	it('asks before switching away from unsaved edits, and stays when told no', async () => {
		const list = [env('dev', { a: '1' }), env('prod')];
		editor.start(list, {});
		editor.setVariables([{ name: 'a', value: 'mine' }]);
		vi.mocked(confirm).mockResolvedValue(false);

		await editor.select('prod', list);

		expect(confirm).toHaveBeenCalledTimes(1);
		expect(editor.draft?.name).toBe('dev');
		expect(editor.draft?.variables).toEqual({ a: 'mine' });
	});

	it('switches without asking when nothing changed', async () => {
		const list = [env('dev'), env('prod')];
		editor.start(list, {});

		await editor.select('prod', list);

		expect(confirm).not.toHaveBeenCalled();
		expect(editor.draft?.name).toBe('prod');
	});

	it('keeps the environment draft while the workspace defaults are shown', async () => {
		const list = [env('dev', { a: '1' })];
		editor.start(list, {});
		editor.setVariables([{ name: 'a', value: 'mine' }]);

		editor.openDefaults();
		await editor.select('dev', list);

		expect(confirm).not.toHaveBeenCalled();
		expect(editor.showDefaults).toBe(false);
		expect(editor.draft?.variables).toEqual({ a: 'mine' });
	});

	it('asks before closing with unsaved environment or default edits', async () => {
		editor.start([env('dev')], {});
		expect(await editor.mayClose()).toBe(true);
		expect(confirm).not.toHaveBeenCalled();

		editor.defaults = [{ name: 'apiVersion', value: '2' }];
		vi.mocked(confirm).mockResolvedValue(false);
		expect(await editor.mayClose()).toBe(false);
		expect(confirm).toHaveBeenCalledTimes(1);
	});

	it('selects a new environment once the list has it, also in an empty workspace', async () => {
		editor.start([], {});
		api.environments.create.mockResolvedValue('staging');
		api.environments.list.mockResolvedValue([env('dev'), env('staging')]);

		await editor.create('staging');

		expect(environments.list.data?.map((e) => e.name)).toEqual(['dev', 'staging']);
		expect(editor.draft?.name).toBe('staging');
	});

	it('picks the first environment when the list arrives after opening', () => {
		editor.start([], {});
		editor.follow([env('dev')]);
		expect(editor.draft?.name).toBe('dev');
	});

	it('follows the file while unedited and flags a change on disk once edited', () => {
		editor.start([env('dev', { a: '1' })], {});
		editor.follow([env('dev', { a: '2' })]);
		expect(editor.draft?.variables).toEqual({ a: '2' });

		editor.setVariables([{ name: 'a', value: 'mine' }]);
		editor.follow([env('dev', { a: '3' })]);
		expect(editor.draft?.variables).toEqual({ a: 'mine' });
		expect(editor.changedOnDisk).toBe(true);

		editor.follow([env('dev', { a: 'mine' })]);
		expect(editor.dirty).toBe(false);
		expect(editor.changedOnDisk).toBe(false);
	});
});
