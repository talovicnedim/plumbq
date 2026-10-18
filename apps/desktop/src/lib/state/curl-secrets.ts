import { toast } from 'svelte-sonner';
import type { CurlSecrets } from '#lib/api/index.js';
import { curlSecretsNotices } from '#lib/utils/curl-secrets.js';

export function toastCurlSecrets(secrets: CurlSecrets, environment: string | null): void {
	for (const notice of curlSecretsNotices(secrets, environment)) {
		if (notice.kind === 'success') toast.success(notice.text);
		else toast.warning(notice.text);
	}
}
