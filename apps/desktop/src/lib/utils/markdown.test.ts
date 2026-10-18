import { describe, expect, it } from 'vitest';
import { renderMarkdown, splitCodeBlocks } from './markdown.js';

describe('markdown', () => {
	it('renders the basics', () => {
		expect(renderMarkdown('# Title\n\nSome **bold** and *it* with `code`.')).toBe(
			'<h1>Title</h1>\n<p>Some <strong>bold</strong> and <em>it</em> with <code>code</code>.</p>'
		);
	});

	it('renders lists and code blocks', () => {
		expect(renderMarkdown('- a\n- b\n\n1. one\n\n```\n<x>\n```')).toBe(
			'<ul><li>a</li><li>b</li></ul>\n<ol><li>one</li></ol>\n<pre><code>&lt;x&gt;</code></pre>'
		);
	});

	it('escapes html and only links http(s) urls', () => {
		const html = renderMarkdown(
			'<script>alert(1)</script> [x](javascript:alert(1)) [ok](https://a.test)'
		);
		expect(html).not.toContain('<script>');
		expect(html).not.toContain('href="javascript');
		expect(html).toContain(
			'<a href="https://a.test" target="_blank" rel="noreferrer noopener">ok</a>'
		);
	});

	it('splits fenced code from prose, so code blocks can be copied', () => {
		expect(splitCodeBlocks('Run this:\n```sh\ncurl -s a.test\n```\nThen check.')).toEqual([
			{ kind: 'prose', text: 'Run this:' },
			{ kind: 'code', text: 'curl -s a.test' },
			{ kind: 'prose', text: 'Then check.' }
		]);
	});

	it('treats an unclosed fence as code while it streams in', () => {
		expect(splitCodeBlocks('```\nline 1\nline')).toEqual([{ kind: 'code', text: 'line 1\nline' }]);
	});
});
