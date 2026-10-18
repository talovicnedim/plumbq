import type { BodyChange } from '#lib/api/index.js';

export function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function formatDuration(ms: number): string {
	return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(2)} s`;
}

/** `2026-10-11T13:41:02.123Z` → a short local time, with the date when not today. */
export function formatTime(iso: string, now = new Date()): string {
	const date = new Date(iso);
	const time = date.toLocaleTimeString([], {
		hour: '2-digit',
		minute: '2-digit',
		second: '2-digit'
	});
	if (date.toDateString() === now.toDateString()) return time;
	return `${date.toLocaleDateString()} ${time}`;
}

/** Last path segment: `/Users/ada/api` → `api`. */
export function baseName(path: string): string {
	return path.split(/[/\\]/).filter(Boolean).pop() ?? path;
}

const CHANGE_SIGN = { added: '+', removed: '-', changed: '~' } as const;

/** A changed JSON field (or text line) as the compare view lists it. */
export function formatBodyChange(change: BodyChange): string {
	const head = `${CHANGE_SIGN[change.kind]} ${change.path}`;
	switch (change.kind) {
		case 'changed':
			return `${head}: ${change.before ?? ''} → ${change.after ?? ''}`;
		case 'added':
			return `${head} = ${change.after ?? ''}`;
		case 'removed':
			return `${head} (was ${change.before ?? ''})`;
	}
}
