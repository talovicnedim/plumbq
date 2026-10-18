import type { VariableHint } from '#lib/api/index.js';

const SECRET = '••••••';

/**
 * A new `{{variable}}` pattern each time: global regexes keep state between uses. Like core's
 * `variables::references`, a variable never spans a line or holds another brace.
 */
export function variablePattern(): RegExp {
	return /\{\{([^{}\n\r]*)\}\}/g;
}

/** Text split into plain and `{{variable}}` parts, for highlighting. */
export function segments(text: string): { text: string; variable: string | null }[] {
	const parts: { text: string; variable: string | null }[] = [];
	let last = 0;
	for (const match of text.matchAll(variablePattern())) {
		const start = match.index ?? 0;
		if (start > last) parts.push({ text: text.slice(last, start), variable: null });
		parts.push({ text: match[0], variable: match[1].trim() });
		last = start + match[0].length;
	}
	if (last < text.length) parts.push({ text: text.slice(last), variable: null });
	return parts;
}

/** The `{{variable}}` in `text` that covers `offset`, braces included. */
export function variableAt(
	text: string,
	offset: number
): { from: number; to: number; name: string } | null {
	for (const match of text.matchAll(variablePattern())) {
		const from = match.index ?? 0;
		const to = from + match[0].length;
		if (offset >= from && offset <= to) return { from, to, name: match[1].trim() };
	}
	return null;
}

interface VariableInfo {
	/** As shown: the plain value, dots for secrets, empty when there is nothing to show. */
	value: string;
	/** Where the value comes from, e.g. `local`, `keychain`, `generated`. */
	source: string;
	missing: boolean;
}

/** What the hover card says about a variable; `environment` is the selected one. */
export function describeVariable(hint: VariableHint, environment: string | null): VariableInfo {
	const found = (value: string, source: string) => ({ value, source, missing: false });
	switch (hint.kind) {
		case 'value':
			return found(hint.value, environment ?? 'environment');
		case 'workspace':
			return found(hint.value, 'workspace default');
		case 'secret':
			return found(SECRET, 'keychain');
		case 'dotenv':
			return found(SECRET, '.env');
		case 'processEnv':
			return found('', 'OS env');
		case 'dynamic':
			return found('', 'generated');
		case 'prompt':
			return found('', 'asked when sending');
		case 'chain':
			return found('', `from last ${hint.value} response`);
		case 'oauth':
			return found(SECRET, `OAuth token for ${hint.value}`);
		case 'file':
			return found(hint.value, 'file variable');
		case 'missing':
			return {
				value: '',
				source: environment ? `not set in ${environment}` : 'no environment picked',
				missing: true
			};
	}
}

/** The one line under the URL when variables are missing, with its link; null when none are. */
export function missingLine(
	hints: VariableHint[],
	environment: string | null
): { text: string; action: string } | null {
	const names = hints.filter((h) => h.kind === 'missing').map((h) => h.name);
	if (names.length === 0) return null;
	const one = names.length === 1;
	const list = listNames(names);
	if (!environment)
		return { text: `${list} ${one ? "isn't" : "aren't"} set.`, action: 'Pick an environment' };
	return {
		text: `${list} ${one ? "isn't" : "aren't"} set in ${environment}.`,
		action: one ? 'Set it' : 'Set them'
	};
}

/** `a`, `a and b`, `a, b and c`, `a, b, c and 2 more`. */
function listNames(names: string[]): string {
	const shown = names.slice(0, 3);
	const more = names.length - shown.length;
	if (more > 0) return `${shown.join(', ')} and ${more} more`;
	if (shown.length === 1) return shown[0];
	return `${shown.slice(0, -1).join(', ')} and ${shown.at(-1)}`;
}
