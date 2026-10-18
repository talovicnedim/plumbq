import type { Header } from '#lib/api/index.js';

export const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];

/** Full class strings so Tailwind can see them. */
export function methodColor(method: string): string {
	switch (method.toUpperCase()) {
		case 'GET':
			return 'text-success';
		case 'POST':
			return 'text-warning';
		case 'PUT':
		case 'PATCH':
			return 'text-info';
		case 'DELETE':
			return 'text-danger';
		default:
			return 'text-special';
	}
}

export function statusColor(status: number): string {
	if (status >= 500) return 'text-danger';
	if (status >= 400) return 'text-warning';
	if (status >= 300) return 'text-info';
	return 'text-success';
}

// Only the characters that would split or end a row when the URL is read back are encoded, so
// typed `%2F` or `{{variables}}` stay exactly as written.
const VALUE_ESCAPES: [string, string][] = [
	['&', '%26'],
	['#', '%23'],
	[' ', '%20']
];
const NAME_ESCAPES: [string, string][] = [...VALUE_ESCAPES, ['=', '%3D']];

/** Query parameters of a URL as editable rows. Keeps `{{variables}}` and other encoding as written. */
export function parseQuery(url: string): Header[] {
	const query = url.split('#')[0].split('?').slice(1).join('?');
	if (!query) return [];
	return query.split('&').map((pair) => {
		const [name, ...value] = pair.split('=');
		return {
			name: unescapeQuery(name, NAME_ESCAPES),
			value: unescapeQuery(value.join('='), VALUE_ESCAPES)
		};
	});
}

/** Replaces a URL's query string with `params`; rows without a name are dropped. */
export function withQuery(url: string, params: Header[]): string {
	const [beforeFragment, ...fragment] = url.split('#');
	const base = beforeFragment.split('?')[0];
	const query = params
		.filter((p) => p.name)
		.map((p) => {
			const name = escapeQuery(p.name, NAME_ESCAPES);
			return p.value ? `${name}=${escapeQuery(p.value, VALUE_ESCAPES)}` : name;
		})
		.join('&');
	const rest = fragment.length ? `#${fragment.join('#')}` : '';
	return (query ? `${base}?${query}` : base) + rest;
}

function escapeQuery(text: string, escapes: [string, string][]): string {
	return escapes.reduce((out, [char, code]) => out.replaceAll(char, code), text);
}

function unescapeQuery(text: string, escapes: [string, string][]): string {
	return escapes.reduce((out, [char, code]) => out.replace(new RegExp(code, 'gi'), char), text);
}

export function headerValue(headers: Header[], name: string): string | undefined {
	return headers.find((h) => h.name.toLowerCase() === name.toLowerCase())?.value;
}

/** Pretty-prints JSON bodies; anything else is returned as is. */
export function prettyBody(body: string, contentType: string | undefined): string {
	const looksJson = contentType?.includes('json') || /^\s*[[{]/.test(body);
	if (!looksJson) return body;
	try {
		return JSON.stringify(JSON.parse(body), null, 2);
	} catch {
		return body;
	}
}
