import { toast } from 'svelte-sonner';
import { api, toApiError, type HistoryEntry } from '#lib/api/index.js';
import { confirm } from './confirm.svelte.js';
import { environments } from './environments.svelte.js';
import { promptValues } from './prompt-values.svelte.js';
import { recentRequests } from './recent-requests.svelte.js';
import type { RequestTab } from './tabs.svelte.js';

/** Last `{{$prompt}}` answers per request, in memory only, to prefill the next ask. */
const answers = new Map<string, Record<string, string>>();

/** Sends a tab's draft, asking for prompts, production confirmation and OAuth sign-in as needed. */
export async function sendTab(tab: RequestTab): Promise<void> {
	if (tab.sending) return;
	tab.sending = true;
	tab.startedAt = Date.now();
	tab.error = undefined;
	tab.cancelled = false;
	try {
		const prompts = await askPrompts(tab);
		if (!prompts) return;
		// Core may ask for a production confirmation or an OAuth sign-in; each is handled once
		// and the send retried.
		let confirmed = false;
		let signedIn = false;
		for (;;) {
			try {
				tab.entry = await send(tab, prompts, confirmed);
				recentRequests.sent(tab.id, tab.entry.response.status);
				return;
			} catch (err) {
				const error = toApiError(err);
				if (error.kind === 'needsConfirmation' && !confirmed) {
					if (!(await confirmProduction(tab))) return;
					confirmed = true;
				} else if (error.kind === 'cancelled') {
					tab.cancelled = true;
					return;
				} else if (error.kind === 'needsAuth' && !signedIn) {
					if (!(await signIn(tab))) return;
					signedIn = true;
				} else {
					tab.error = error.message;
					return;
				}
			}
		}
	} catch (err) {
		tab.error = toApiError(err).message;
	} finally {
		tab.sending = false;
	}
}

/** Asks for every `{{$prompt name}}` value; `null` when the user cancels. */
async function askPrompts(tab: RequestTab): Promise<Record<string, string> | null> {
	const names = await api.requests.prompts(tab.id, tab.draft);
	if (names.length === 0) return {};
	const given = await promptValues.ask(
		`Values for "${tab.draft.name || tab.id}"`,
		names,
		answers.get(tab.id) ?? {}
	);
	if (given) answers.set(tab.id, given);
	return given;
}

function send(
	tab: RequestTab,
	prompts: Record<string, string>,
	confirmed: boolean
): Promise<HistoryEntry> {
	return api.requests.send(tab.id, tab.draft, environments.selected, prompts, confirmed);
}

function confirmProduction(tab: RequestTab): Promise<boolean> {
	const env = environments.selected ?? '';
	return confirm({
		title: `Send ${tab.draft.method} to production?`,
		description: `This may change data in "${env}".`,
		confirmLabel: `Send ${tab.draft.method}`,
		destructive: true,
		typeToConfirm: env
	});
}

/** Runs the browser sign-in for the OAuth client the request is waiting for. */
async function signIn(tab: RequestTab): Promise<boolean> {
	const env = environments.selected;
	const name = await api.auth.signInNeeded(tab.id, tab.draft, env ?? null);
	if (!name || !env) return false;
	const ok = await confirm({
		title: `Sign in to ${name}?`,
		description:
			'Your browser opens to sign in. Come back here when you are done; the request is sent right after.',
		confirmLabel: 'Sign in'
	});
	if (!ok) return false;
	const waiting = toast.loading(`Waiting for sign-in to ${name} in the browser…`);
	try {
		await api.auth.signIn(env, name);
		toast.success(`Signed in to ${name}`, { id: waiting });
		void environments.list.refresh();
		return true;
	} catch (err) {
		toast.error(toApiError(err).message, { id: waiting });
		return false;
	}
}
