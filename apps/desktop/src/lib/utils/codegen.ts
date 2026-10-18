import type { CodeLanguage } from '#lib/api/index.js';

export const CODE_LANGUAGES: { id: CodeLanguage; label: string }[] = [
	{ id: 'curl', label: 'cURL' },
	{ id: 'fetch', label: 'JavaScript (fetch)' },
	{ id: 'python', label: 'Python (requests)' },
	{ id: 'laravel', label: 'PHP (Laravel Http)' },
	{ id: 'go', label: 'Go (net/http)' },
	{ id: 'httpie', label: 'HTTPie' }
];
