import { toast } from 'svelte-sonner';
import { api, toApiError, type BodyFile } from '#lib/api/index.js';

/**
 * Lets the user pick a file inside the workspace to send in request `requestId`'s body; core
 * returns it relative to the `.http` file. Null when cancelled or refused (shown as a toast).
 */
export async function chooseBodyFile(requestId: string): Promise<BodyFile | null> {
	try {
		return await api.bodyFiles.choose(requestId);
	} catch (err) {
		toast.error(toApiError(err).message);
		return null;
	}
}
