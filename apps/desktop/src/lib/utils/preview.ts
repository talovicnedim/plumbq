import type { Header, Response } from '#lib/api/index.js';
import { headerValue } from './http.js';

export type PreviewKind = 'image' | 'pdf' | 'html' | 'audio' | 'video';

/** How the response body is shown. */
export type BodyMode = 'pretty' | 'raw' | 'preview';

/** How a response can be previewed, from its Content-Type; `null` for plain data. */
export function previewKind(contentType: string | undefined): PreviewKind | null {
	const type = (contentType ?? '').split(';')[0].trim().toLowerCase();
	if (type.startsWith('image/')) return 'image';
	if (type === 'application/pdf') return 'pdf';
	if (type === 'text/html' || type === 'application/xhtml+xml') return 'html';
	if (type.startsWith('audio/')) return 'audio';
	if (type.startsWith('video/')) return 'video';
	return null;
}

/** The body as bytes in a Blob, for previews of binary (or SVG) responses. */
export function bodyBlob(response: Response, contentType: string): Blob {
	if (!response.binary) return new Blob([response.body], { type: contentType });
	const raw = atob(response.bodyBase64 ?? '');
	const bytes = new Uint8Array(raw.length);
	for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
	return new Blob([bytes], { type: contentType });
}

const EXTENSIONS: Record<string, string> = {
	'application/json': 'json',
	'application/xml': 'xml',
	'text/xml': 'xml',
	'text/html': 'html',
	'text/plain': 'txt',
	'text/csv': 'csv',
	'application/pdf': 'pdf',
	'image/png': 'png',
	'image/jpeg': 'jpg',
	'image/gif': 'gif',
	'image/webp': 'webp',
	'image/svg+xml': 'svg',
	'application/zip': 'zip',
	'application/octet-stream': 'bin'
};

/** A file name for "save response": Content-Disposition, else the URL's last segment, with an
 * extension from the Content-Type when it has none. */
export function suggestFilename(headers: Header[], url: string): string {
	const disposition = headerValue(headers, 'content-disposition') ?? '';
	const quoted = disposition.match(/filename\*?=(?:UTF-8'')?"?([^";]+)"?/i)?.[1];
	if (quoted) return safeDecode(quoted.trim());

	const path = url.split(/[?#]/)[0].replace(/^[a-z]+:\/\/[^/]+/i, '');
	const last = path.split('/').filter(Boolean).pop() ?? 'response';
	if (/\.[a-z0-9]{1,5}$/i.test(last)) return last;
	const type = (headerValue(headers, 'content-type') ?? '').split(';')[0].trim().toLowerCase();
	const ext = EXTENSIONS[type] ?? (type.endsWith('+json') ? 'json' : 'txt');
	return `${last}.${ext}`;
}

/** Bytes of all headers as sent on the wire (`Name: value\r\n`), for the size display. */
export function headersSize(headers: Header[]): number {
	return headers.reduce(
		(sum, h) => sum + new TextEncoder().encode(`${h.name}: ${h.value}\r\n`).length,
		0
	);
}

/** `decodeURIComponent` throws on stray `%` (`filename="100%.txt"`); fall back to the raw name. */
function safeDecode(text: string): string {
	try {
		return decodeURIComponent(text);
	} catch {
		return text;
	}
}
