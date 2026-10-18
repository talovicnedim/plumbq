import { describe, expect, it } from 'vitest';
import { PROVIDERS, providerMeta } from './providers.js';

describe('providers', () => {
	it('offers the six tiles with Custom last', () => {
		expect(PROVIDERS.map((p) => p.label)).toEqual([
			'Ollama',
			'LM Studio',
			'Anthropic',
			'OpenAI',
			'Gemini',
			'Custom'
		]);
		expect(providerMeta('openAiCompatible')?.where).toBe('any URL');
		expect(PROVIDERS.filter((p) => p.where === 'cloud').every((p) => p.keyUrl)).toBe(true);
	});
});
