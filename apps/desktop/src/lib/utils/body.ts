import type { Header } from '#lib/api/index.js';
import { headerValue } from './http.js';

export const BODY_KINDS = ['none', 'json', 'form', 'multipart', 'file', 'text', 'xml'] as const;

export type BodyKind = (typeof BODY_KINDS)[number];

export const BODY_LABEL: Record<BodyKind, string> = {
	none: 'None',
	json: 'JSON',
	form: 'Form',
	multipart: 'Multipart',
	file: 'File',
	text: 'Text',
	xml: 'XML'
};

const CONTENT_TYPE: Record<Exclude<BodyKind, 'none' | 'multipart'>, string> = {
	json: 'application/json',
	form: 'application/x-www-form-urlencoded',
	file: 'application/octet-stream',
	text: 'text/plain',
	xml: 'application/xml'
};

/**
 * The body kind implied by the Content-Type header, falling back to the body itself.
 * `filePath` is the file a `< ./path` body sends, as core reads it.
 */
export function bodyKind(headers: Header[], body: string, filePath: string | null): BodyKind {
	const type = headerValue(headers, 'content-type')?.toLowerCase() ?? '';
	if (type.startsWith('multipart/')) return 'multipart';
	if (filePath !== null) return 'file';
	if (type.includes('json')) return 'json';
	if (type.includes('x-www-form-urlencoded')) return 'form';
	if (type.includes('xml')) return 'xml';
	if (type) return 'text';
	if (!body.trim()) return 'none';
	return /^\s*[[{]/.test(body) ? 'json' : 'text';
}

/** Sets the Content-Type for `kind`; `none` removes it. Multipart gets a fresh boundary. */
export function withBodyKind(headers: Header[], kind: BodyKind): Header[] {
	const rest = headers.filter((h) => h.name.toLowerCase() !== 'content-type');
	if (kind === 'none') return rest;
	const value =
		kind === 'multipart' ? `multipart/form-data; boundary=${newBoundary()}` : CONTENT_TYPE[kind];
	return [...rest, { name: 'Content-Type', value }];
}

function newBoundary(): string {
	return `----plumbq${Math.random().toString(36).slice(2, 12)}`;
}

/** The boundary of a multipart Content-Type header. */
export function boundaryOf(headers: Header[]): string | null {
	const type = headerValue(headers, 'content-type') ?? '';
	const match = type.match(/boundary="?([^";]+)"?/i);
	return type.toLowerCase().startsWith('multipart/') && match ? match[1] : null;
}

/** `a=1&b=two` as rows. Values stay encoded as written. */
export function parseForm(body: string): Header[] {
	if (!body.trim()) return [];
	return body
		.trim()
		.split('&')
		.map((pair) => {
			const [name, ...value] = pair.split('=');
			return { name, value: value.join('=') };
		});
}

export function formBody(rows: Header[]): string {
	return rows
		.filter((r) => r.name)
		.map((r) => `${r.name}=${r.value}`)
		.join('&');
}
