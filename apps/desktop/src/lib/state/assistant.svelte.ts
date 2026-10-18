import { toast } from 'svelte-sonner';
import {
	api,
	toApiError,
	type AgentEvent,
	type AgentContext,
	type AssistantStatus,
	type ChatMessage,
	type ChatSummary,
	type ProviderConfig,
	type ProviderStatus,
	type QuickAction
} from '#lib/api/index.js';
import { applyEvent, itemsFromMessages, newChatId, type ChatItem } from '#lib/utils/chat.js';
import { ai } from './ai.svelte.js';
import { confirm } from './confirm.svelte.js';
import { environments } from './environments.svelte.js';
import { Query } from './query.svelte.js';
import { tabs } from './tabs.svelte.js';

const OPEN_KEY = 'plumbq.assistant.open';

/** What the user attached to their next question. */
export interface ContextChips {
	request: boolean;
	folder: boolean;
	response: boolean;
}

/** The chat panel: the current chat, its stream, and provider settings. */
class AssistantState {
	open = $state(localStorage.getItem(OPEN_KEY) === '1');
	status = $state<AssistantStatus>();
	chatId = $state(newChatId());
	/** The conversation as sent to the model. */
	messages = $state<ChatMessage[]>([]);
	/** The conversation as shown, including live tool cards. */
	items = $state<ChatItem[]>([]);
	sending = $state(false);
	error = $state<string>();
	readonly chats = new Query<ChatSummary[]>(() => api.assistant.chats.list());
	quickActions = $state<QuickAction[]>([]);
	chips = $state<ContextChips>({ request: true, folder: false, response: false });
	/** A question from "Explain this" waiting for a model to be picked. */
	pending = $state<string>();

	readonly provider: ProviderConfig | undefined = $derived(
		this.status?.settings.providers.find((p) => p.id === this.status?.settings.activeProvider)
	);
	readonly providerStatus: ProviderStatus | undefined = $derived(
		this.status?.providers.find((p) => p.id === this.provider?.id)
	);
	/**
	 * A cloud provider the user hasn't accepted "what gets sent" for yet (the Chat tab asks once
	 * when its tile is picked).
	 */
	readonly needsNotice = $derived(
		!!this.provider &&
			!this.providerStatus?.local &&
			!this.status?.settings.acknowledged.includes(this.provider.id)
	);
	/** A model is picked, keyed if it needs one, and AI is on: questions can go out. */
	readonly ready = $derived(
		!!this.provider?.model &&
			!this.needsNotice &&
			!ai.off &&
			(!this.providerStatus?.needsKey || this.providerStatus.hasKey)
	);

	toggle(): void {
		this.open = !this.open;
		localStorage.setItem(OPEN_KEY, this.open ? '1' : '0');
		if (this.open) void this.load();
	}

	/** Opens the panel and asks `text` now, or once a model is set up. */
	askWhenReady(text: string): void {
		if (!this.open) this.toggle();
		if (this.ready) void this.ask(text);
		else this.pending = text;
	}

	/** New settings (from the AI dialog or here); a waiting question goes out once ready. */
	setStatus(status: AssistantStatus): void {
		this.status = status;
		const pending = this.pending;
		if (pending && this.ready && !this.sending) {
			this.pending = undefined;
			void this.ask(pending);
		}
	}

	async load(): Promise<void> {
		try {
			this.setStatus(await api.assistant.status());
			void this.chats.refresh();
			if (this.quickActions.length === 0) {
				this.quickActions = await api.assistant.quickActions();
			}
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async ask(text: string): Promise<void> {
		const question = text.trim();
		if (!question || this.sending) return;
		this.error = undefined;
		const messages: ChatMessage[] = [
			...this.messages,
			{
				role: 'user',
				content: question,
				toolCalls: [],
				toolCallId: null,
				toolName: null,
				isError: false
			}
		];
		this.items = [...this.items, { kind: 'user', text: question }];
		this.sending = true;
		const chatId = this.chatId;
		// Pieces can arrive after the turn ends; what it returns (or saved) already holds them.
		let ended = false;
		const onEvent = (event: AgentEvent) => {
			if (!ended && this.chatId === chatId) applyEvent(this.items, event);
		};
		try {
			const chat = await api.assistant
				.send(chatId, messages, this.context(), onEvent)
				.finally(() => (ended = true));
			if (this.chatId !== chatId) return;
			this.messages = chat.messages;
			this.items = itemsFromMessages(chat.messages);
		} catch (err) {
			if (this.chatId !== chatId) return;
			this.error = toApiError(err).message;
			await this.reload(chatId, messages);
		} finally {
			this.sending = false;
			void this.chats.refresh();
		}
	}

	/**
	 * After a failed turn: the saved chat keeps the tool calls that ran, so the next question
	 * builds on it. A turn refused before it started saved nothing, so its chat is older.
	 */
	private async reload(chatId: string, sent: ChatMessage[]): Promise<void> {
		const saved = await api.assistant.chats.get(chatId).catch(() => undefined);
		if (this.chatId !== chatId) return;
		if (!saved || saved.messages.length < sent.length) {
			this.messages = sent;
			return;
		}
		this.messages = saved.messages;
		this.items = itemsFromMessages(saved.messages);
	}

	async runQuickAction(action: QuickAction): Promise<void> {
		try {
			await this.ask(await api.assistant.quickAction(action.name, tabs.active?.id ?? null));
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	stop(): void {
		api.assistant.stop(this.chatId).catch((err) => toast.error(toApiError(err).message));
	}

	newChat(): void {
		if (this.sending) this.stop();
		this.chatId = newChatId();
		this.messages = [];
		this.items = [];
		this.error = undefined;
	}

	/** Another workspace has its own chats: start a new one there and drop a waiting question. */
	reset(): void {
		this.newChat();
		this.pending = undefined;
		void this.chats.refresh();
	}

	async openChat(id: string): Promise<void> {
		try {
			const chat = await api.assistant.chats.get(id);
			this.chatId = chat.id;
			this.messages = chat.messages;
			this.items = itemsFromMessages(chat.messages);
			this.error = undefined;
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async removeChat(id: string, title: string): Promise<void> {
		const ok = await confirm({
			title: 'Delete chat?',
			description: `"${title}" is deleted for good.`,
			confirmLabel: 'Delete',
			destructive: true
		});
		if (!ok) return;
		try {
			await api.assistant.chats.remove(id);
		} catch (err) {
			toast.error(toApiError(err).message);
			return;
		}
		if (id === this.chatId) this.newChat();
		void this.chats.refresh();
	}

	/** What the user is looking at, as far as they attached it. */
	private context(): AgentContext {
		const active = tabs.active;
		return {
			workspace: '',
			environment: environments.selected,
			requestId: this.chips.request && active ? active.id : null,
			folder: this.chips.folder && active ? folderOf(active.id) : null,
			historyId: this.chips.response && active?.entry ? active.entry.id : null
		};
	}
	/**
	 * "Explain this" and friends: opens the chat and asks `prompt` about what the user is looking
	 * at (`attach` picks what goes along). Without a model yet, the chat shows its setup and asks
	 * once one is picked.
	 */
	explain(prompt: string, attach: Partial<ContextChips> = {}): void {
		if (ai.off) return;
		this.chips = { ...this.chips, ...attach };
		this.askWhenReady(prompt);
	}
}

/** `users/admin/list.http#2` → `users/admin`. */
function folderOf(id: string): string {
	const path = id.split('#')[0];
	return path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '';
}

export const assistant = new AssistantState();
