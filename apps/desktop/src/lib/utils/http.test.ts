import { describe, expect, it } from 'vitest';
import { headerValue, parseQuery, prettyBody, withQuery } from './http.js';

describe('query params', () => {
	it('parses params keeping variables', () => {
		expect(parseQuery('{{base}}/users?page=2&q={{term}}&flag')).toEqual([
			{ name: 'page', value: '2' },
			{ name: 'q', value: '{{term}}' },
			{ name: 'flag', value: '' }
		]);
	});

	it('returns no params without a query', () => {
		expect(parseQuery('https://x.test/a')).toEqual([]);
	});

	it('rebuilds the url and drops empty rows', () => {
		const url = withQuery('https://x.test/a?old=1', [
			{ name: 'a', value: '1' },
			{ name: '', value: 'x' },
			{ name: 'flag', value: '' }
		]);
		expect(url).toBe('https://x.test/a?a=1&flag');
	});

	it('removes the question mark when no params remain', () => {
		expect(withQuery('https://x.test/a?old=1', [])).toBe('https://x.test/a');
	});

	it('keeps &, # and spaces typed in a value', () => {
		const url = withQuery('https://x.test/a', [{ name: 'q', value: 'tom & jerry #1' }]);
		expect(url).toBe('https://x.test/a?q=tom%20%26%20jerry%20%231');
		expect(parseQuery(url)).toEqual([{ name: 'q', value: 'tom & jerry #1' }]);
	});

	it('keeps = typed in a name', () => {
		const url = withQuery('https://x.test/a', [{ name: 'a=b', value: '1' }]);
		expect(parseQuery(url)).toEqual([{ name: 'a=b', value: '1' }]);
	});

	it('leaves other encoding and variables as written', () => {
		const url = withQuery('{{base}}/a', [{ name: 'path', value: '%2Fhome {{dir}}' }]);
		expect(url).toBe('{{base}}/a?path=%2Fhome%20{{dir}}');
		expect(parseQuery(url)).toEqual([{ name: 'path', value: '%2Fhome {{dir}}' }]);
	});

	it('keeps the fragment when the query changes', () => {
		const url = withQuery('https://x.test/a?old=1#top', [{ name: 'page', value: '2' }]);
		expect(url).toBe('https://x.test/a?page=2#top');
	});
});

describe('bodies and headers', () => {
	it('pretty-prints json', () => {
		expect(prettyBody('{"a":1}', 'application/json')).toBe('{\n  "a": 1\n}');
	});

	it('leaves invalid json and text alone', () => {
		expect(prettyBody('{oops', 'application/json')).toBe('{oops');
		expect(prettyBody('hello', 'text/plain')).toBe('hello');
	});

	it('finds headers case-insensitively', () => {
		expect(headerValue([{ name: 'Content-Type', value: 'x' }], 'content-type')).toBe('x');
	});
});
