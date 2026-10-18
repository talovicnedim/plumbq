import { toApiError, api, type RequestView } from '#lib/api/index.js';
import { environments } from './environments.svelte.js';
import type { RequestTab } from './tabs.svelte.js';

/** How long typing pauses before core looks at the draft again. */
const WAIT_MS = 150;

/**
 * What core says about a tab's unsaved draft (variable hints, tags, docs, body warnings),
 * fetched again shortly after each edit or environment change.
 */
export class RequestViewState {
	data = $state<RequestView>();
	error = $state<string>();
	#timer: ReturnType<typeof setTimeout> | undefined;
	#latest = 0;

	/** Asks core again after a short pause; only the newest answer is kept. */
	refresh(tab: RequestTab): void {
		clearTimeout(this.#timer);
		const draft = $state.snapshot(tab.draft);
		const id = tab.id;
		const environment = environments.selected;
		const ticket = ++this.#latest;
		this.#timer = setTimeout(async () => {
			try {
				const view = await api.requests.inspect(id, draft, environment);
				if (ticket !== this.#latest) return;
				this.data = view;
				this.error = undefined;
			} catch (err) {
				if (ticket === this.#latest) this.error = toApiError(err).message;
			}
		}, WAIT_MS);
	}

	stop(): void {
		clearTimeout(this.#timer);
	}
}
