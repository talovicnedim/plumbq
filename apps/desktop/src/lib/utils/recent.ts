export interface RecentRequest {
	id: string;
	/** `Date.now()` of the last open or send. */
	at: number;
	/** Status of the last send, when it was sent. */
	status?: number;
	sentAt?: number;
}

/** How many requests the switcher remembers per workspace. */
export const MAX_RECENT = 20;

/** Moves `id` to the front, merging what is known about it; oldest entries fall off. */
export function touchRecent(
	list: RecentRequest[],
	id: string,
	patch: Omit<Partial<RecentRequest>, 'id'> = {},
	now = Date.now()
): RecentRequest[] {
	const existing = list.find((r) => r.id === id);
	const entry: RecentRequest = { ...existing, ...patch, id, at: now };
	return [entry, ...list.filter((r) => r.id !== id)].slice(0, MAX_RECENT);
}

/** Applies id changes from moves and renames, dropping entries of removed requests. */
export function renameRecent(
	list: RecentRequest[],
	changes: { from: string; to: string | null }[]
): RecentRequest[] {
	const renamed = new Map(changes.map((c) => [c.from, c.to]));
	return list.flatMap((r) => {
		if (!renamed.has(r.id)) return [r];
		const to = renamed.get(r.id);
		return to ? [{ ...r, id: to }] : [];
	});
}
