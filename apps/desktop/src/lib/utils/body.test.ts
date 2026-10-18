import { describe, expect, it } from 'vitest';
import { bodyKind, boundaryOf, formBody, parseForm, withBodyKind } from './body.js';

describe('body', () => {
	it('detects kind from content type, then from the body', () => {
		expect(bodyKind([{ name: 'content-type', value: 'application/vnd.api+json' }], '', null)).toBe(
			'json'
		);
		expect(bodyKind([], '{"a":1}', null)).toBe('json');
		expect(bodyKind([], 'hello', null)).toBe('text');
		expect(bodyKind([], '  ', null)).toBe('none');
	});

	it('sets and removes the content type', () => {
		const headers = withBodyKind([{ name: 'Content-Type', value: 'text/plain' }], 'form');
		expect(headers).toEqual([{ name: 'Content-Type', value: 'application/x-www-form-urlencoded' }]);
		expect(withBodyKind(headers, 'none')).toEqual([]);
	});

	it('round-trips form bodies', () => {
		const rows = parseForm('a=1&b={{x}}&c=x=y');
		expect(rows).toEqual([
			{ name: 'a', value: '1' },
			{ name: 'b', value: '{{x}}' },
			{ name: 'c', value: 'x=y' }
		]);
		expect(formBody([...rows, { name: '', value: 'skip' }])).toBe('a=1&b={{x}}&c=x=y');
	});
});

describe('files and multipart', () => {
	it('puts multipart first, then the file core found', () => {
		const json = [{ name: 'Content-Type', value: 'application/json' }];
		expect(bodyKind(json, '< ./a.json', './a.json')).toBe('file');
		expect(bodyKind(json, '< ./a.json', null)).toBe('json');
		expect(
			bodyKind([{ name: 'Content-Type', value: 'multipart/form-data; boundary=B' }], '', null)
		).toBe('multipart');
	});

	it('reads boundaries and sets a new one', () => {
		const headers = withBodyKind([], 'multipart');
		expect(boundaryOf(headers)).toMatch(/^----plumbq/);
		expect(
			boundaryOf([{ name: 'content-type', value: 'multipart/form-data; boundary="x y"' }])
		).toBe('x y');
	});
});
