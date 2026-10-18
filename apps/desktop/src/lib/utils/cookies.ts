/** A cookie's expiry (RFC 3339, or none for session cookies) for display. */
export function formatExpiry(expires: string | null, now = new Date()): string {
	if (expires === null) return 'session';
	const date = new Date(expires);
	if (date.getTime() <= now.getTime()) return 'expired';
	return date.toDateString() === now.toDateString()
		? date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
		: date.toLocaleDateString();
}
