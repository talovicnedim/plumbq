import type { IdChange } from '#lib/api/index.js';
import { renameRecent, touchRecent, type RecentRequest } from '#lib/utils/recent.js';

const PREFIX = 'plumbq.recent:';

/** Recently opened and sent requests of the open workspace, for ⌘E and the ⌘K palette. */
class RecentRequestsState {
	list = $state<RecentRequest[]>([]);
	#root: string | null = null;

	/** Loads the list of a workspace; called when the workspace opens. */
	load(root: string | null): void {
		this.#root = root;
		if (!root) {
			this.list = [];
			return;
		}
		try {
			const saved: unknown = JSON.parse(localStorage.getItem(PREFIX + root) ?? '[]');
			this.list = Array.isArray(saved) ? saved.filter(isRecent) : [];
		} catch {
			this.list = [];
		}
	}

	opened(id: string): void {
		this.#save(touchRecent(this.list, id));
	}

	sent(id: string, status: number): void {
		this.#save(touchRecent(this.list, id, { status, sentAt: Date.now() }));
	}

	follow(changes: IdChange[]): void {
		this.#save(renameRecent(this.list, changes));
	}

	#save(next: RecentRequest[]): void {
		this.list = next;
		if (this.#root) localStorage.setItem(PREFIX + this.#root, JSON.stringify(next));
	}
}

function isRecent(value: unknown): value is RecentRequest {
	return (
		typeof value === 'object' &&
		value !== null &&
		'id' in value &&
		typeof value.id === 'string' &&
		'at' in value &&
		typeof value.at === 'number'
	);
}

export const recentRequests = new RecentRequestsState();
