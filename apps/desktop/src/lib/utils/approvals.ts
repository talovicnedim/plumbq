import type { ApprovalRequest } from '#lib/api/index.js';

/**
 * Whether "Allow once" can be pressed: strong requests need their name typed. Core checks the
 * typed name again and trims it the same way, so a trailing space doesn't keep the button off.
 */
export function canAllow(
	request: Pick<ApprovalRequest, 'strong' | 'confirmName'>,
	typed: string
): boolean {
	return !request.strong || typed.trim() === (request.confirmName ?? '');
}
