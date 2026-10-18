/**
 * A tiny markdown renderer for request and folder docs: headings, paragraphs, lists, code
 * blocks, inline code, bold, italic and links. Everything is HTML-escaped first, so the output
 * is safe to insert with `{@html}` whatever the docs contain.
 */

function escape(text: string): string {
	return text
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;')
		.replace(/'/g, '&#39;');
}

function inline(text: string): string {
	return escape(text)
		.replace(/`([^`]+)`/g, '<code>$1</code>')
		.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
		.replace(/(^|[^*])\*([^*]+)\*/g, '$1<em>$2</em>')
		.replace(
			/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
			'<a href="$2" target="_blank" rel="noreferrer noopener">$1</a>'
		);
}

export function renderMarkdown(source: string): string {
	const out: string[] = [];
	const lines = source.replace(/\r\n/g, '\n').split('\n');
	let paragraph: string[] = [];
	let list: { ordered: boolean; items: string[] } | null = null;

	const flush = () => {
		if (paragraph.length) out.push(`<p>${paragraph.map(inline).join('<br>')}</p>`);
		paragraph = [];
		if (list) {
			const tag = list.ordered ? 'ol' : 'ul';
			out.push(`<${tag}>${list.items.map((i) => `<li>${inline(i)}</li>`).join('')}</${tag}>`);
		}
		list = null;
	};

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];
		if (line.trim().startsWith('```')) {
			flush();
			const code: string[] = [];
			while (++i < lines.length && !lines[i].trim().startsWith('```')) code.push(lines[i]);
			out.push(`<pre><code>${escape(code.join('\n'))}</code></pre>`);
			continue;
		}
		const heading = line.match(/^(#{1,6})\s+(.*)$/);
		const bullet = line.match(/^\s*[-*]\s+(.*)$/);
		const numbered = line.match(/^\s*\d+[.)]\s+(.*)$/);
		if (heading) {
			flush();
			const level = heading[1].length;
			out.push(`<h${level}>${inline(heading[2])}</h${level}>`);
		} else if (bullet || numbered) {
			const ordered = Boolean(numbered);
			if (paragraph.length || (list && list.ordered !== ordered)) flush();
			list ??= { ordered, items: [] };
			list.items.push((bullet ?? numbered)?.[1] ?? '');
		} else if (!line.trim()) {
			flush();
		} else {
			if (list) flush();
			paragraph.push(line.trim());
		}
	}
	flush();
	return out.join('\n');
}

export interface MarkdownPart {
	kind: 'prose' | 'code';
	text: string;
}

/** Splits fenced code blocks from the prose around them, so each block gets a copy button. */
export function splitCodeBlocks(source: string): MarkdownPart[] {
	const parts: MarkdownPart[] = [];
	const lines = source.replace(/\r\n/g, '\n').split('\n');
	let prose: string[] = [];
	const flushProse = () => {
		const text = prose.join('\n').trim();
		if (text) parts.push({ kind: 'prose', text });
		prose = [];
	};
	for (let i = 0; i < lines.length; i++) {
		if (!lines[i].trim().startsWith('```')) {
			prose.push(lines[i]);
			continue;
		}
		flushProse();
		const code: string[] = [];
		while (++i < lines.length && !lines[i].trim().startsWith('```')) code.push(lines[i]);
		parts.push({ kind: 'code', text: code.join('\n') });
	}
	flushProse();
	return parts;
}
