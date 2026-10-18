import type { Header } from '#lib/api/index.js';

/**
 * Rows as editable text, one per line: `Name: value` for headers, `name=value` for params and
 * form fields.
 */
export function toBulk(rows: Header[], separator: ':' | '='): string {
	const join = separator === ':' ? ': ' : '=';
	return rows.map((row) => `${row.name}${join}${row.value}`).join('\n');
}

/** The inverse of `toBulk`; blank lines are skipped, a line without separator is a name. */
export function fromBulk(text: string, separator: ':' | '='): Header[] {
	return text
		.split('\n')
		.filter((line) => line.trim())
		.map((line) => {
			const at = line.indexOf(separator);
			if (at === -1) return { name: line.trim(), value: '' };
			return { name: line.slice(0, at).trim(), value: line.slice(at + 1).trim() };
		});
}
