import type { BodyMode } from './preview.js';

/** The part of a Content-Type a view choice is remembered by: `application/json`, `image/*`. */
export function modeKey(contentType: string | undefined): string {
	const base = (contentType ?? '').split(';')[0].trim().toLowerCase();
	if (!base) return 'unknown';
	const [type] = base.split('/');
	// Images, audio and video share one choice per family; other types are remembered exactly.
	return ['image', 'audio', 'video'].includes(type) ? `${type}/*` : base;
}

export function isBodyMode(value: unknown): value is BodyMode {
	return value === 'pretty' || value === 'raw' || value === 'preview';
}
