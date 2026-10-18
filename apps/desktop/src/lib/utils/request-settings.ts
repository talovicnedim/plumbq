import type { RequestSettings } from '#lib/api/index.js';

/** Whether a request turns on any setting; core reads missing ones as off or unset. */
export function hasRequestSettings(settings: RequestSettings | undefined): boolean {
	return Object.values(settings ?? {}).some((value) => value !== false && value != null);
}
