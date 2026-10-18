import { describe, expect, it } from 'vitest';
import { authFromHeaders, withAuth } from './auth.js';

describe('auth', () => {
	it('reads bearer tokens', () => {
		const auth = authFromHeaders([{ name: 'Authorization', value: 'Bearer {{token}}' }]);
		expect(auth).toEqual({ kind: 'bearer', value: '{{token}}', header: '' });
	});

	it('writes basic auth readable, as core encodes it at send time', () => {
		const headers = withAuth([], { kind: 'basic', value: 'ada:pw', header: '' });
		expect(headers).toEqual([{ name: 'Authorization', value: 'Basic ada:pw' }]);
		expect(authFromHeaders(headers).value).toBe('ada:pw');
		const vars = withAuth([], { kind: 'basic', value: '{{user}}:{{pw}}', header: '' });
		expect(vars[0].value).toBe('Basic {{user}}:{{pw}}');
	});

	it('still reads base64 basic auth from older files', () => {
		const auth = authFromHeaders([{ name: 'Authorization', value: 'Basic YWRhOnB3' }]);
		expect(auth.value).toBe('ada:pw');
	});

	it('replaces the previous auth and keeps other headers', () => {
		const before = [
			{ name: 'Accept', value: 'application/json' },
			{ name: 'X-API-Key', value: 'k' }
		];
		const after = withAuth(before, { kind: 'bearer', value: 't', header: '' });
		expect(after).toEqual([
			{ name: 'Accept', value: 'application/json' },
			{ name: 'Authorization', value: 'Bearer t' }
		]);
	});

	it('none removes auth headers', () => {
		const headers = [{ name: 'Authorization', value: 'Bearer t' }];
		expect(withAuth(headers, { kind: 'none', value: '', header: '' })).toEqual([]);
	});

	it('reads and writes oauth tokens', () => {
		const headers = withAuth([], { kind: 'oauth2', value: 'github', header: '' });
		expect(headers).toEqual([{ name: 'Authorization', value: 'Bearer {{$auth.token("github")}}' }]);
		expect(authFromHeaders(headers)).toEqual({ kind: 'oauth2', value: 'github', header: '' });
		expect(
			authFromHeaders([{ name: 'Authorization', value: 'Bearer {{$auth.token api}}' }]).value
		).toBe('api');
	});
});
