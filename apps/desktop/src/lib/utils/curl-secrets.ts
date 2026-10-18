import type { CurlSecrets } from '#lib/api/index.js';

export interface CurlSecretsNotice {
	kind: 'success' | 'warning';
	text: string;
}

/** What to tell the user after a cURL paste kept (or could not keep) its credentials. */
export function curlSecretsNotices(
	secrets: CurlSecrets,
	environment: string | null
): CurlSecretsNotice[] {
	const notices: CurlSecretsNotice[] = [];
	if (secrets.saved.length > 0) {
		notices.push({
			kind: 'success',
			text: `Saved ${list(secrets.saved)} as ${noun(secrets.saved)} in ${environment}`
		});
	}
	if (secrets.kept.length > 0) {
		notices.push({
			kind: 'warning',
			text: `${list(secrets.kept)} ${secrets.kept.length === 1 ? 'is' : 'are'} already ${noun(secrets.kept)} in ${environment}, so ${secrets.kept.length === 1 ? 'its value was' : 'their values were'} not changed`
		});
	}
	if (secrets.unsaved.length > 0) {
		notices.push({
			kind: 'warning',
			text: `Pick an environment (or create "local") and set ${list(secrets.unsaved)} as ${noun(secrets.unsaved)}. The request uses them as placeholders`
		});
	}
	return notices;
}

function noun(names: string[]): string {
	return names.length === 1 ? 'a secret' : 'secrets';
}

function list(names: string[]): string {
	if (names.length <= 1) return names.join('');
	return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
}
