import type { ProviderKind } from '#lib/api/index.js';

interface ProviderMeta {
	kind: ProviderKind;
	label: string;
	/** Under the label on the tile. */
	where: 'local' | 'cloud' | 'any URL';
	/** Two letters on a gradient, in place of a logo. */
	mark: string;
	gradient: string;
	/** Where to get a key, linked next to the key field. */
	keyUrl?: string;
}

/** The six tiles of the Chat tab, in order. Models aren't listed here: they come from the
 * provider, since they change all the time. */
export const PROVIDERS: ProviderMeta[] = [
	{
		kind: 'ollama',
		label: 'Ollama',
		where: 'local',
		mark: 'Ol',
		gradient: 'from-provider-ollama to-provider-ollama-deep'
	},
	{
		kind: 'lmStudio',
		label: 'LM Studio',
		where: 'local',
		mark: 'LM',
		gradient: 'from-provider-lm-studio to-provider-lm-studio-deep'
	},
	{
		kind: 'anthropic',
		label: 'Anthropic',
		where: 'cloud',
		mark: 'An',
		gradient: 'from-provider-anthropic to-provider-anthropic-deep',
		keyUrl: 'https://console.anthropic.com/settings/keys'
	},
	{
		kind: 'openAi',
		label: 'OpenAI',
		where: 'cloud',
		mark: 'Oa',
		gradient: 'from-provider-openai to-provider-openai-deep',
		keyUrl: 'https://platform.openai.com/api-keys'
	},
	{
		kind: 'gemini',
		label: 'Gemini',
		where: 'cloud',
		mark: 'Ge',
		gradient: 'from-provider-gemini to-provider-gemini-deep',
		keyUrl: 'https://aistudio.google.com/apikey'
	},
	{
		kind: 'openAiCompatible',
		label: 'Custom',
		where: 'any URL',
		mark: '{}',
		gradient: 'from-provider-custom to-provider-custom-deep'
	}
];

export function providerMeta(kind: ProviderKind | undefined): ProviderMeta | undefined {
	return PROVIDERS.find((p) => p.kind === kind);
}
