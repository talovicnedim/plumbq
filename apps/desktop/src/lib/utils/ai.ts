import type { ActivityEntry } from '#lib/api/index.js';

/** Who did something in the activity log: the built-in chat or an AI app connected over MCP. */
interface ActivitySource {
	label: string;
	chat: boolean;
}

export function activitySource(entry: ActivityEntry): ActivitySource {
	if (entry.fromChat) return { label: 'Chat', chat: true };
	return { label: entry.actor || 'AI app', chat: false };
}

/** What the "Explain this" buttons ask the chat. Ids let it find the request with its tools. */
export const ASK = {
	sendFailed: (id: string, error: string) =>
		`Sending request ${id} failed with: "${error}". Look at the request and its environment, explain the likely cause and how to fix it.`,
	responseFailed: (id: string, status: number) =>
		`Request ${id} got a ${status} response. Look at the request and the response, explain why it failed and how to fix it.`,
	response: (id: string) =>
		`Explain the last response of request ${id}: what it contains and anything notable.`,
	assertion: (id: string, expression: string, message: string) =>
		`The assertion \`${expression}\` on request ${id} failed: ${message}. Explain why, and whether the assertion or the API is wrong.`,
	writeTests: (id: string) =>
		`Write assertions for request ${id} based on its responses, and add them to the request.`,
	document: (id: string) =>
		`Document request ${id}: what it does, its parameters and its responses, as the request's docs.`
};
