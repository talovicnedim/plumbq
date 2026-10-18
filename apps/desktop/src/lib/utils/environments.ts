import type { EnvironmentTag } from '#lib/api/index.js';

/** Display order: most dangerous first. */
export const ENVIRONMENT_TAGS: EnvironmentTag[] = ['production', 'staging', 'development', 'local'];

export const ENVIRONMENT_LABEL: Record<EnvironmentTag, string> = {
	production: 'Production',
	staging: 'Staging',
	development: 'Development',
	local: 'Local'
};

export const ENVIRONMENT_SHORT: Record<EnvironmentTag, string> = {
	production: 'PROD',
	staging: 'STG',
	development: 'DEV',
	local: 'LOCAL'
};

/** Full class strings so Tailwind can see them. */
export const ENVIRONMENT_BG: Record<EnvironmentTag, string> = {
	production: 'bg-env-production',
	staging: 'bg-env-staging',
	development: 'bg-env-development',
	local: 'bg-env-local'
};
