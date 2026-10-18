import { describe, expect, it } from 'vitest';
import type { VariableHint, VariableKind } from '#lib/api/index.js';
import { describeVariable, missingLine, segments, variableAt } from './variables.js';

const hint = (name: string, kind: VariableKind, value = ''): VariableHint => ({
	name,
	kind,
	value
});

describe('variables', () => {
	it('splits text into highlightable segments', () => {
		expect(segments('{{base}}/a')).toEqual([
			{ text: '{{base}}', variable: 'base' },
			{ text: '/a', variable: null }
		]);
	});

	it('reads variables like core: never across lines or with another brace inside', () => {
		expect(segments('/{{xy\n{"a": 1}\n{{key}}').map((s) => s.variable)).toEqual([null, 'key']);
		expect(segments('{{{token}}}')).toEqual([
			{ text: '{', variable: null },
			{ text: '{{token}}', variable: 'token' },
			{ text: '}', variable: null }
		]);
	});

	it('finds the variable under an offset, braces included', () => {
		const text = 'a {{ token }} b';
		expect(variableAt(text, 2)).toEqual({ from: 2, to: 13, name: 'token' });
		expect(variableAt(text, 13)).toEqual({ from: 2, to: 13, name: 'token' });
		expect(variableAt(text, 1)).toBeNull();
		expect(variableAt(text, 14)).toBeNull();
	});

	it('names where a value comes from and never shows secrets', () => {
		expect(describeVariable(hint('baseUrl', 'value', 'http://x'), 'local')).toEqual({
			value: 'http://x',
			source: 'local',
			missing: false
		});
		expect(describeVariable(hint('apiKey', 'secret'), 'local').value).toBe('••••••');
		expect(describeVariable(hint('$dotenv KEY', 'dotenv'), null).value).toBe('••••••');
		expect(describeVariable(hint('$auth.token("gh")', 'oauth', 'gh'), null)).toEqual({
			value: '••••••',
			source: 'OAuth token for gh',
			missing: false
		});
		expect(describeVariable(hint('login.response.body.$.t', 'chain', 'login'), null).source).toBe(
			'from last login response'
		);
		expect(describeVariable(hint('v', 'workspace', '1'), 'local').source).toBe('workspace default');
	});

	it('says where a missing variable is missing', () => {
		expect(describeVariable(hint('token', 'missing'), 'local')).toEqual({
			value: '',
			source: 'not set in local',
			missing: true
		});
		expect(describeVariable(hint('token', 'missing'), null).source).toBe('no environment picked');
	});

	it('writes one line about missing variables, only when some are', () => {
		const set = hint('baseUrl', 'value', 'http://x');
		expect(missingLine([set], 'local')).toBeNull();
		expect(missingLine([set, hint('apiKey', 'missing')], 'local')).toEqual({
			text: "apiKey isn't set in local.",
			action: 'Set it'
		});
		expect(missingLine([hint('apiKey', 'missing'), hint('token', 'missing')], 'local')).toEqual({
			text: "apiKey and token aren't set in local.",
			action: 'Set them'
		});
		expect(missingLine([hint('apiKey', 'missing')], null)).toEqual({
			text: "apiKey isn't set.",
			action: 'Pick an environment'
		});
		const many = ['a', 'b', 'c', 'd', 'e'].map((n) => hint(n, 'missing'));
		expect(missingLine(many, 'local')?.text).toBe("a, b, c and 2 more aren't set in local.");
		expect(missingLine(many.slice(0, 3), 'local')?.text).toBe("a, b and c aren't set in local.");
	});
});
