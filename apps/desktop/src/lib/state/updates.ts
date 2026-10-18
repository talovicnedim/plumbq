import { toast } from 'svelte-sonner';
import { api, toApiError } from '#lib/api/index.js';
import { confirmDiscardAll } from './quit.js';

/**
 * Checks for a newer release and offers to install it. Only signed release builds have the
 * updater; elsewhere (dev, unsigned builds, offline) `check` fails. The launch check stays
 * silent then; a manual check (`{ manual: true }`, from Settings) says what happened.
 */
export async function checkForUpdates({ manual = false } = {}): Promise<void> {
	if (import.meta.env.DEV) {
		if (manual) toast.info("Development builds don't update themselves.");
		return;
	}
	let update;
	try {
		update = await api.updates.check();
	} catch (err) {
		if (manual) toast.error(`Couldn't check for updates: ${toApiError(err).message}`);
		return;
	}
	if (!update) {
		if (manual) toast.success('You have the latest version.');
		return;
	}
	const available = update;
	toast(`Version ${available.version} is available`, {
		description: 'Restart to update.',
		duration: Infinity,
		action: {
			label: 'Restart to update',
			onClick: async () => {
				// Restarting drops unsaved tabs; ask like quitting does.
				if (!(await confirmDiscardAll('Restart'))) return;
				const installing = toast.loading('Downloading the update…');
				try {
					await available.downloadAndInstall();
					await api.app.relaunch();
				} catch (err) {
					toast.error(`Update failed: ${toApiError(err).message}`, { id: installing });
				}
			}
		}
	});
}
