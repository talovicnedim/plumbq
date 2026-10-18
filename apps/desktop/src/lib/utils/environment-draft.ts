import type { Environment } from '#lib/api/index.js';

/**
 * What an environment looks like on disk: everything but keychain state (`isSet`,
 * `signedIn`), which changes without the file changing.
 */
export function fileKey(env: Environment): string {
	return JSON.stringify({
		tag: env.tag,
		variables: env.variables,
		secrets: env.secrets.map((s) => s.name),
		auth: env.auth.map(({ signedIn: _, ...client }) => client)
	});
}

type DraftSync = 'inSync' | 'reload' | 'conflict';

/**
 * How a draft relates to a fresh copy from disk. `base` is the fileKey of the copy the draft
 * started from. Unedited drafts follow the disk; edited ones are flagged when the file changed
 * underneath.
 */
export function syncDraft(draft: Environment, fresh: Environment, base: string): DraftSync {
	const freshKey = fileKey(fresh);
	if (freshKey === fileKey(draft)) return 'inSync';
	if (freshKey === base) return 'inSync';
	return fileKey(draft) === base ? 'reload' : 'conflict';
}
