import { toast } from 'svelte-sonner';
import {
	api,
	toApiError,
	type ApprovalAnswer,
	type ApprovalRequest,
	type McpBinary,
	type McpSettings,
	type McpStatus
} from '#lib/api/index.js';
import { assistant } from './assistant.svelte.js';

export type AiTab = 'chat' | 'apps' | 'permissions' | 'activity';

/**
 * The AI dialog (one place for the chat's model, AI apps over MCP, permissions and activity),
 * the in-app MCP server with the AI requests waiting for approval, and "Turn AI off", which
 * stops both doors at once.
 */
class AiState {
	dialogOpen = $state(false);
	tab = $state<AiTab>('chat');
	status = $state<McpStatus>();
	binary = $state<McpBinary>();
	/** Oldest first; the approval dialog shows the first. */
	pending = $state<ApprovalRequest[]>([]);

	readonly running = $derived(this.status?.url != null);
	/** Until the settings load, AI counts as on, so nothing flickers away. */
	readonly off = $derived(this.status?.settings.turnedOff ?? false);

	open(tab: AiTab = 'chat'): void {
		this.tab = tab;
		this.dialogOpen = true;
		void assistant.load();
		void this.load();
	}

	async load(): Promise<void> {
		try {
			this.status = await api.mcp.status();
			await this.refreshPending();
			if (!this.binary) this.binary = await api.mcp.binary();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async update(change: Partial<McpSettings>): Promise<void> {
		if (!this.status) return;
		try {
			this.status = await api.mcp.update({ ...this.status.settings, ...change });
			if (this.status.error) toast.error(this.status.error);
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async regenerateToken(): Promise<void> {
		try {
			this.status = await api.mcp.token.regenerate();
			toast.success('New address created. AI apps set up with the old one need the new one.');
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	async refreshPending(): Promise<void> {
		try {
			this.pending = await api.mcp.approvals.list();
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}

	/** Keeps the request waiting when the answer is refused (e.g. a wrongly typed name). */
	async answer(id: string, answer: ApprovalAnswer, typed: string | null): Promise<boolean> {
		try {
			this.status = await api.mcp.approvals.answer(id, answer, typed);
		} catch (err) {
			toast.error(toApiError(err).message);
			return false;
		}
		this.pending = this.pending.filter((p) => p.id !== id);
		await this.refreshPending();
		return true;
	}

	/** Stops the chat and the MCP server until AI is turned on again. */
	async turnOff(): Promise<void> {
		await this.update({ turnedOff: true });
		if (assistant.open) assistant.toggle();
	}

	/** Brings the chat back, and the MCP server if it is enabled. */
	async turnOn(): Promise<void> {
		await this.update({ turnedOff: false });
	}
}

export const ai = new AiState();
