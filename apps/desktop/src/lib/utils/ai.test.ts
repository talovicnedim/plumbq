import { describe, expect, it } from 'vitest';
import type { ActivityEntry } from '#lib/api/index.js';
import { ASK, activitySource } from './ai.js';

describe('activitySource', () => {
	const entry = (actor: string, fromChat: boolean): ActivityEntry => ({
		at: '2026-10-13T10:00:00Z',
		actor,
		fromChat,
		workspace: '/ws',
		tool: 'workspace_tree',
		environment: null,
		permission: 'readWorkspace',
		outcome: 'ran',
		summary: 'tree'
	});

	it('tells the chat from AI apps by the typed flag, not the name', () => {
		expect(activitySource(entry('Chat · Claude Sonnet 5.5', true))).toEqual({
			label: 'Chat',
			chat: true
		});
		expect(activitySource(entry('claude-code', false))).toEqual({
			label: 'claude-code',
			chat: false
		});
		expect(activitySource(entry('Chat · fake', false))).toEqual({
			label: 'Chat · fake',
			chat: false
		});
		expect(activitySource(entry('', false))).toEqual({ label: 'AI app', chat: false });
	});
});

describe('ASK prompts', () => {
	it('name the request so the chat can look it up', () => {
		expect(ASK.responseFailed('users/list.http', 404)).toContain('users/list.http');
		expect(ASK.responseFailed('users/list.http', 404)).toContain('404');
		expect(ASK.assertion('a.http', 'status == 200', 'got 500')).toContain('`status == 200`');
	});
});
