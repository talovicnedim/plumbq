import { describe, expect, it } from 'vitest';
import { curlSecretsNotices } from './curl-secrets.js';

const none = { saved: [], kept: [], unsaved: [] };

describe('curlSecretsNotices', () => {
	it('says nothing when the command had no credentials', () => {
		expect(curlSecretsNotices(none, 'local')).toEqual([]);
	});

	it('names the saved secrets and their environment', () => {
		const notices = curlSecretsNotices({ ...none, saved: ['cookie', 'token'] }, 'local');
		expect(notices).toEqual([
			{ kind: 'success', text: 'Saved cookie and token as secrets in local' }
		]);
	});

	it('warns about secrets the environment already had', () => {
		const [notice] = curlSecretsNotices({ ...none, kept: ['token'] }, 'local');
		expect(notice.kind).toBe('warning');
		expect(notice.text).toBe('token is already a secret in local, so its value was not changed');
	});

	it('asks to pick an environment when none was selected', () => {
		const [notice] = curlSecretsNotices({ ...none, unsaved: ['a', 'b', 'c'] }, null);
		expect(notice.kind).toBe('warning');
		expect(notice.text).toContain('Pick an environment');
		expect(notice.text).toContain('a, b and c');
	});
});
