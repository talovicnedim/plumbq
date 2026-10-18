import type { AgentEvent, ChatMessage } from '#lib/api/index.js';

/** What the chat panel shows: messages and tool calls in order. */
export type ChatItem =
	| { kind: 'user'; text: string }
	| { kind: 'assistant'; text: string }
	| {
			kind: 'tool';
			id: string;
			name: string;
			summary: string;
			status: 'running' | 'ok' | 'error';
			/** The start of the result. */
			excerpt: string;
	  };

/** A saved conversation as panel items: tool calls paired with their results. */
export function itemsFromMessages(messages: ChatMessage[]): ChatItem[] {
	const items: ChatItem[] = [];
	for (const message of messages) {
		if (message.role === 'user') items.push({ kind: 'user', text: message.content });
		if (message.role === 'assistant') {
			if (message.content) items.push({ kind: 'assistant', text: message.content });
			for (const call of message.toolCalls ?? []) {
				items.push({
					kind: 'tool',
					id: call.id,
					name: call.name,
					summary: summarize(call.arguments),
					status: 'running',
					excerpt: ''
				});
			}
		}
		if (message.role === 'tool') {
			const card = items.find((i) => i.kind === 'tool' && i.id === message.toolCallId);
			if (card?.kind === 'tool') {
				card.status = message.isError ? 'error' : 'ok';
				card.excerpt = message.content.slice(0, 400);
			}
		}
	}
	return items;
}

/**
 * Folds one streamed event into the items, in place: a text piece changes only the last item,
 * so earlier messages don't render their Markdown again.
 */
export function applyEvent(items: ChatItem[], event: AgentEvent): void {
	switch (event.type) {
		case 'text': {
			const last = items.at(-1);
			if (last?.kind === 'assistant') last.text += event.text;
			else items.push({ kind: 'assistant', text: event.text });
			return;
		}
		case 'toolStarted':
			items.push({
				kind: 'tool',
				id: event.id,
				name: event.name,
				summary: event.summary,
				status: 'running',
				excerpt: ''
			});
			return;
		case 'toolFinished': {
			const card = items.find((item) => item.kind === 'tool' && item.id === event.id);
			if (card?.kind !== 'tool') return;
			card.status = event.ok ? 'ok' : 'error';
			card.excerpt = event.excerpt;
			return;
		}
		case 'done':
			items.splice(0, items.length, ...itemsFromMessages(event.messages));
	}
}

/** The most telling argument of a tool call, like the agent does for live cards. */
export function summarize(argumentsJson: string): string {
	try {
		const args: unknown = JSON.parse(argumentsJson);
		if (typeof args !== 'object' || args === null) return '';
		for (const key of [
			'id',
			'request_id',
			'folder',
			'path',
			'query',
			'url',
			'name',
			'environment'
		]) {
			const value: unknown = key in args ? Reflect.get(args, key) : undefined;
			if (typeof value === 'string' && value) return value;
		}
	} catch {
		// Malformed arguments from a model just get no summary.
	}
	return '';
}

/** `requests_send` → `Requests send`, for tool cards. */
export function toolLabel(name: string): string {
	const text = name.replace(/_/g, ' ');
	return text.charAt(0).toUpperCase() + text.slice(1);
}

/** Chat ids are plain alphanumerics (the backend refuses anything else). */
export function newChatId(): string {
	return crypto.randomUUID().replace(/-/g, '');
}
