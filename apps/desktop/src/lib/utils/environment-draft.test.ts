import { describe, expect, it } from 'vitest';
import type { Environment } from '#lib/api/index.js';
import { fileKey, syncDraft } from './environment-draft.js';

const env = (vars: Record<string, string>, isSet = false): Environment => ({
	name: 'dev',
	tag: 'local',
	variables: vars,
	secrets: [{ name: 'token', isSet }],
	auth: []
});

describe('environment drafts', () => {
	it('ignores keychain state', () => {
		expect(fileKey(env({}, true))).toBe(fileKey(env({}, false)));
	});

	it('reloads unedited drafts and flags edited ones when the file changed', () => {
		const base = fileKey(env({ a: '1' }));
		expect(syncDraft(env({ a: '1' }), env({ a: '2' }), base)).toBe('reload');
		expect(syncDraft(env({ a: 'mine' }), env({ a: '2' }), base)).toBe('conflict');
		expect(syncDraft(env({ a: 'mine' }), env({ a: '1' }), base)).toBe('inSync');
		expect(syncDraft(env({ a: '2' }), env({ a: '2' }), base)).toBe('inSync');
	});
});
