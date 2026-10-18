import { describe, expect, it } from 'vitest';
import { headersSize, previewKind, suggestFilename } from './preview.js';

describe('preview', () => {
	it('detects preview kinds from content type', () => {
		expect(previewKind('image/png')).toBe('image');
		expect(previewKind('image/svg+xml; charset=utf-8')).toBe('image');
		expect(previewKind('application/pdf')).toBe('pdf');
		expect(previewKind('text/html; charset=utf-8')).toBe('html');
		expect(previewKind('audio/mpeg')).toBe('audio');
		expect(previewKind('video/mp4')).toBe('video');
		expect(previewKind('application/json')).toBeNull();
		expect(previewKind(undefined)).toBeNull();
	});

	it('suggests file names', () => {
		const disposition = [
			{ name: 'Content-Disposition', value: 'attachment; filename="report 1.pdf"' }
		];
		expect(suggestFilename(disposition, 'https://x.test/download')).toBe('report 1.pdf');
		expect(suggestFilename([], 'https://x.test/files/logo.png?v=2')).toBe('logo.png');
		const json = [{ name: 'Content-Type', value: 'application/json; charset=utf-8' }];
		expect(suggestFilename(json, 'https://x.test/users/7')).toBe('7.json');
		expect(suggestFilename([], 'https://x.test/')).toBe('response.txt');
	});

	it('measures headers as sent', () => {
		expect(headersSize([{ name: 'A', value: 'bc' }])).toBe('A: bc\r\n'.length);
	});

	it('keeps malformed percent-encoded names instead of failing', () => {
		const headers = [{ name: 'Content-Disposition', value: 'attachment; filename="100%.txt"' }];
		expect(suggestFilename(headers, 'https://x.test/a')).toBe('100%.txt');
	});
});
