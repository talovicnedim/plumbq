import { describe, expect, it } from 'vitest';
import type { ChatMessage } from '#lib/api/index.js';
import {
	applyEvent,
	itemsFromMessages,
	newChatId,
	summarize,
	toolLabel,
	type ChatItem
} from './chat.js';

const call = { id: 'c1', name: 'workspace_tree', arguments: '{"folder":"users"}' };

describe('chat view', () => {
	it('streams text into the last bubble in place', () => {
		const items: ChatItem[] = [{ kind: 'user', text: 'hi' }];
		const question = items[0];
		applyEvent(items, { type: 'text', text: 'Hel' });
		const answer = items[1];
		applyEvent(items, { type: 'text', text: 'lo' });
		expect(items).toEqual([
			{ kind: 'user', text: 'hi' },
			{ kind: 'assistant', text: 'Hello' }
		]);
		expect(items[0]).toBe(question);
		expect(items[1]).toBe(answer);
	});

	it('shows tool cards running, then finished', () => {
		const items: ChatItem[] = [];
		applyEvent(items, { type: 'toolStarted', id: 'c1', name: 'workspace_tree', summary: '' });
		const card = items[0];
		expect(card).toMatchObject({ kind: 'tool', status: 'running' });
		applyEvent(items, { type: 'toolFinished', id: 'c1', ok: false, excerpt: 'denied' });
		expect(items[0]).toBe(card);
		expect(card).toMatchObject({ status: 'error', excerpt: 'denied' });
	});

	it('rebuilds saved conversations with tool results', () => {
		const messages: ChatMessage[] = [
			{
				role: 'user',
				content: 'what is here?',
				toolCalls: [],
				toolCallId: null,
				toolName: null,
				isError: false
			},
			{
				role: 'assistant',
				content: '',
				toolCalls: [call],
				toolCallId: null,
				toolName: null,
				isError: false
			},
			{
				role: 'tool',
				content: 'users/list.http',
				toolCalls: [],
				toolCallId: 'c1',
				toolName: 'workspace_tree',
				isError: false
			},
			{
				role: 'assistant',
				content: 'One request.',
				toolCalls: [],
				toolCallId: null,
				toolName: null,
				isError: false
			}
		];
		const items = itemsFromMessages(messages);
		expect(items.map((i) => i.kind)).toEqual(['user', 'tool', 'assistant']);
		expect(items[1]).toMatchObject({ status: 'ok', summary: 'users', excerpt: 'users/list.http' });
		const streamed: ChatItem[] = [{ kind: 'user', text: 'stale' }];
		applyEvent(streamed, { type: 'done', messages });
		expect(streamed).toEqual(items);
	});

	it('summarizes and labels tools', () => {
		expect(summarize('{"query":"login"}')).toBe('login');
		expect(summarize('not json')).toBe('');
		expect(toolLabel('requests_send')).toBe('Requests send');
		expect(newChatId()).toMatch(/^[0-9a-f]{32}$/);
	});
});
