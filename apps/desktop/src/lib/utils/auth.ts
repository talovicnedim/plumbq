import type { Header } from '#lib/api/index.js';

export const AUTH_KINDS = ['none', 'bearer', 'basic', 'apiKey', 'oauth2'] as const;

export type AuthKind = (typeof AUTH_KINDS)[number];

export interface Auth {
	kind: AuthKind;
	/** Bearer token, API key value, `user:password` for basic, or the OAuth client name. */
	value: string;
	/** Header name for API keys. */
	header: string;
}

export const AUTH_LABEL: Record<AuthKind, string> = {
	none: 'None',
	bearer: 'Bearer token',
	basic: 'Basic auth',
	apiKey: 'API key',
	oauth2: 'OAuth 2.0'
};

const OAUTH_TOKEN = /^\{\{\s*\$auth\.token\s*\(?\s*["']?([^"')}]+?)["']?\s*\)?\s*\}\}$/;

const API_KEY_HEADERS = ['x-api-key', 'api-key', 'apikey', 'x-auth-token'];

/** Reads auth from headers, so the `.http` file stays the only source of truth. */
export function authFromHeaders(headers: Header[]): Auth {
	const authorization = headers.find((h) => h.name.toLowerCase() === 'authorization');
	if (authorization) {
		const [scheme, ...rest] = authorization.value.split(' ');
		const value = rest.join(' ');
		const oauth = value.match(OAUTH_TOKEN);
		if (scheme.toLowerCase() === 'bearer' && oauth)
			return { kind: 'oauth2', value: oauth[1], header: '' };
		if (scheme.toLowerCase() === 'bearer') return { kind: 'bearer', value, header: '' };
		if (scheme.toLowerCase() === 'basic')
			return { kind: 'basic', value: decodeBasic(value), header: '' };
	}
	const apiKey = headers.find((h) => API_KEY_HEADERS.includes(h.name.toLowerCase()));
	if (apiKey) return { kind: 'apiKey', value: apiKey.value, header: apiKey.name };
	return { kind: 'none', value: '', header: '' };
}

/** Replaces the auth header(s) in `headers` with the ones for `auth`. */
export function withAuth(headers: Header[], auth: Auth): Header[] {
	const current = authFromHeaders(headers);
	const drop = (h: Header) =>
		h.name.toLowerCase() === 'authorization' ||
		(current.kind === 'apiKey' && h.name === current.header);
	const rest = headers.filter((h) => !drop(h));
	switch (auth.kind) {
		case 'bearer':
			return [...rest, { name: 'Authorization', value: `Bearer ${auth.value}` }];
		case 'basic':
			return [...rest, { name: 'Authorization', value: `Basic ${auth.value}` }];
		case 'apiKey':
			return [...rest, { name: auth.header || 'X-API-Key', value: auth.value }];
		case 'oauth2':
			return [...rest, { name: 'Authorization', value: `Bearer {{$auth.token("${auth.value}")}}` }];
		case 'none':
			return rest;
	}
}

/** Core base64-encodes readable credentials at send time; older files may hold them encoded. */
function decodeBasic(value: string): string {
	if (value.includes('{{') || value.includes(':')) return value;
	try {
		return atob(value);
	} catch {
		return value;
	}
}
