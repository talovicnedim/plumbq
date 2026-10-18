import { getVersion } from '@tauri-apps/api/app';
import { Channel } from '@tauri-apps/api/core';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';
import {
	commands,
	type AgentContext,
	type AgentEvent,
	type AgentEvent_Deserialize,
	type AppError,
	type ApprovalAnswer,
	type AssistantSettings,
	type ChatMessage,
	type OpenKind,
	type CodeLanguage,
	type Environment,
	type EnvironmentTag,
	type McpApp,
	type McpSettings,
	type MultipartField,
	type ProviderKind,
	type FolderSettings_Deserialize,
	type Request,
	type Response,
	type RunOutcome,
	type RunReport,
	type RunnerRequest
} from './bindings.js';

export type * from './bindings.js';

/** Error thrown by every `api.*` call. `kind` is stable; `message` is for humans. */
export class ApiError extends Error {
	readonly kind: string;

	constructor({ kind, message }: AppError) {
		super(message);
		this.kind = kind;
	}
}

type CommandResult<T> = { status: 'ok'; data: T } | { status: 'error'; error: AppError };

async function unwrap<T>(result: Promise<CommandResult<T>>): Promise<T> {
	const res = await result;
	if (res.status === 'error') throw new ApiError(res.error);
	return res.data;
}

/** The only way components talk to the backend. Mirrors core: `api.<resource>.<verb>()`. */
export const api = {
	workspace: {
		open: (path: string) => unwrap(commands.workspaceOpen(path)),
		create: (parent: string, name: string) => unwrap(commands.workspaceCreate(parent, name)),
		openDemo: () => unwrap(commands.workspaceOpenDemo()),
		current: () => commands.workspaceCurrent(),
		/** Sets the name shown for the open workspace; empty shows the folder's name again. */
		rename: (name: string) => unwrap(commands.workspaceRename(name)),
		recent: () => unwrap(commands.workspaceRecent()),
		tree: () => unwrap(commands.workspaceTree()),
		warnings: () => unwrap(commands.workspaceWarnings()),
		setPfxPassword: (password: string) => unwrap(commands.workspaceSetPfxPassword(password)),
		hasPfxPassword: () => unwrap(commands.workspaceHasPfxPassword()),
		httpDefaults: () => commands.workspaceHttpDefaults()
	},
	requests: {
		get: (id: string) => unwrap(commands.requestsGet(id)),
		create: (folder: string, request: Request) => unwrap(commands.requestsCreate(folder, request)),
		/** `expected` is the request's `source` as last read; a mismatch fails with kind `conflict`. */
		update: (id: string, request: Request, expected: string | null) =>
			unwrap(commands.requestsUpdate(id, request, expected)),
		remove: (id: string, expected: string | null) => unwrap(commands.requestsRemove(id, expected)),
		move: (id: string, folder: string, position: number | null, expected: string | null) =>
			unwrap(commands.requestsMove(id, folder, position, expected)),
		inspect: (id: string, request: Request, environment: string | null) =>
			unwrap(commands.requestsInspect(id, request, environment)),
		tags: {
			update: (comments: string[], tags: string[]) => commands.requestsTagsUpdate(comments, tags)
		},
		docs: {
			update: (comments: string[], docs: string) => commands.requestsDocsUpdate(comments, docs)
		},
		prompts: (id: string, request: Request) => unwrap(commands.requestsPrompts(id, request)),
		cancel: (id: string) => commands.requestsCancel(id),
		send: (
			id: string,
			request: Request,
			environment: string | null,
			prompts: Record<string, string>,
			confirmed: boolean
		) => unwrap(commands.requestsSend(id, request, environment, prompts, confirmed)),
		export: (id: string, request: Request, environment: string | null, language: CodeLanguage) =>
			unwrap(commands.requestsExport(id, request, environment, language)),
		importCurl: (folder: string, name: string, command: string, environment: string | null) =>
			unwrap(commands.requestsImportCurl(folder, name, command, environment))
	},
	auth: {
		signIn: (environment: string, name: string) => unwrap(commands.authSignIn(environment, name)),
		client: {
			new: (name: string) => commands.authClientNew(name)
		},
		signInNeeded: (id: string, request: Request, environment: string | null) =>
			unwrap(commands.authSignInNeeded(id, request, environment)),
		signOut: (environment: string, name: string) => unwrap(commands.authSignOut(environment, name))
	},
	cookies: {
		list: (environment: string | null) => unwrap(commands.cookiesList(environment)),
		clear: (environment: string | null) => unwrap(commands.cookiesClear(environment)),
		remove: (environment: string | null, domain: string, name: string) =>
			unwrap(commands.cookiesRemove(environment, domain, name))
	},
	curl: {
		parse: (command: string, environment: string | null) =>
			unwrap(commands.curlParse(command, environment))
	},
	bodyFiles: {
		/** Asks for a file inside the workspace; `null` when cancelled. */
		choose: (requestId: string) => unwrap(commands.bodyFilesChoose(requestId))
	},
	multipart: {
		parse: (body: string, boundary: string) => commands.multipartParse(body, boundary),
		format: (fields: MultipartField[], boundary: string) =>
			commands.multipartFormat(fields, boundary)
	},
	folders: {
		create: (parent: string, name: string) => unwrap(commands.foldersCreate(parent, name)),
		rename: (path: string, name: string) => unwrap(commands.foldersRename(path, name)),
		duplicate: (path: string) => unwrap(commands.foldersDuplicate(path)),
		remove: (path: string) => unwrap(commands.foldersRemove(path)),
		reorder: (path: string, order: string[]) => unwrap(commands.foldersReorder(path, order)),
		getSettings: (path: string) => unwrap(commands.foldersGetSettings(path)),
		updateSettings: (path: string, settings: FolderSettings_Deserialize) =>
			unwrap(commands.foldersUpdateSettings(path, settings))
	},
	environments: {
		list: () => unwrap(commands.environmentsList()),
		warnings: () => unwrap(commands.environmentsWarnings()),
		create: (name: string, tag: EnvironmentTag) => unwrap(commands.environmentsCreate(name, tag)),
		update: (environment: Environment) => unwrap(commands.environmentsUpdate(environment)),
		remove: (name: string) => unwrap(commands.environmentsRemove(name)),
		setSecret: (environment: string, name: string, value: string) =>
			unwrap(commands.environmentsSetSecret(environment, name, value)),
		removeSecret: (environment: string, name: string) =>
			unwrap(commands.environmentsRemoveSecret(environment, name))
	},
	assistant: {
		status: () => unwrap(commands.assistantStatus()),
		update: (settings: AssistantSettings) => unwrap(commands.assistantUpdate(settings)),
		provider: {
			new: (kind: ProviderKind, name: string) => commands.assistantProviderNew(kind, name)
		},
		key: {
			set: (providerId: string, key: string) => unwrap(commands.assistantKeySet(providerId, key))
		},
		models: (providerId: string) => unwrap(commands.assistantModels(providerId)),
		test: (providerId: string) => unwrap(commands.assistantTest(providerId)),
		quickActions: () => unwrap(commands.assistantQuickActions()),
		quickAction: (name: string, requestId: string | null) =>
			unwrap(commands.assistantQuickAction(name, requestId)),
		/** Streams the turn's progress to `onEvent`; resolves with the saved chat. */
		send: (
			chatId: string,
			messages: ChatMessage[],
			context: AgentContext,
			onEvent: (event: AgentEvent) => void
		) => {
			const events = new Channel<AgentEvent_Deserialize>();
			events.onmessage = onEvent;
			return unwrap(commands.assistantSend(chatId, messages, context, events));
		},
		stop: (chatId: string) => commands.assistantStop(chatId),
		chats: {
			list: () => unwrap(commands.assistantChatsList()),
			get: (id: string) => unwrap(commands.assistantChatsGet(id)),
			remove: (id: string) => unwrap(commands.assistantChatsRemove(id))
		}
	},
	mcp: {
		binary: () => unwrap(commands.mcpBinary()),
		status: () => commands.mcpStatus(),
		update: (settings: McpSettings) => unwrap(commands.mcpUpdate(settings)),
		token: {
			regenerate: () => unwrap(commands.mcpTokenRegenerate())
		},
		apps: {
			list: () => unwrap(commands.mcpAppsList()),
			connect: (app: McpApp) => unwrap(commands.mcpAppsConnect(app)),
			disconnect: (app: McpApp) => unwrap(commands.mcpAppsDisconnect(app))
		},
		clients: {
			list: () => unwrap(commands.mcpClientsList())
		},
		approvals: {
			list: () => commands.mcpApprovalsList(),
			answer: (id: string, answer: ApprovalAnswer, typed: string | null) =>
				unwrap(commands.mcpApprovalsAnswer(id, answer, typed))
		},
		activity: {
			list: (limit?: number) => unwrap(commands.mcpActivityList(limit ?? null)),
			clear: () => unwrap(commands.mcpActivityClear())
		}
	},
	examples: {
		list: (id: string) => unwrap(commands.examplesList(id)),
		create: (id: string, name: string, response: Response) =>
			unwrap(commands.examplesCreate(id, name, response)),
		remove: (id: string, name: string) => unwrap(commands.examplesRemove(id, name))
	},
	compare: {
		bodies: (before: string, after: string) => commands.compareBodies(before, after)
	},
	responses: {
		/** Asks where to save the body; resolves with the path, or `null` when cancelled. */
		export: (response: Response, fileName: string) =>
			unwrap(commands.responsesExport(response, fileName))
	},
	jsonPath: {
		query: (body: string, path: string) => unwrap(commands.jsonPathQuery(body, path))
	},
	history: {
		list: (requestId: string) => unwrap(commands.historyList(requestId)),
		get: (requestId: string, id: string) => unwrap(commands.historyGet(requestId, id))
	},
	import: {
		file: (folder: string, path: string) => unwrap(commands.importFile(folder, path)),
		url: (folder: string, url: string) => unwrap(commands.importUrl(folder, url))
	},
	runner: {
		preview: (target: string, tags: string[], environment: string | null) =>
			unwrap(commands.runnerPreview(target, tags, environment)),
		/** Streams each request's outcome to `onOutcome`; resolves with the report. */
		run: (run: RunnerRequest, onOutcome: (outcome: RunOutcome) => void) => {
			const outcomes = new Channel<RunOutcome>();
			outcomes.onmessage = onOutcome;
			return unwrap(commands.runnerRun(run, outcomes));
		},
		stop: () => commands.runnerStop(),
		exportReport: (report: RunReport, fileName: string) =>
			unwrap(commands.runnerExportReport(report, fileName))
	},
	export: {
		postman: (folder: string, fileName: string) => unwrap(commands.exportPostman(folder, fileName)),
		docs: (environment: string | null, fileName: string) =>
			unwrap(commands.exportDocs(environment, fileName))
	},
	/** Native dialogs opened by the backend; only paths picked here can be imported or opened. */
	dialog: {
		open: async (kind: OpenKind): Promise<string | null> => {
			const result = await commands.dialogOpen(kind);
			return result.status === 'ok' ? result.data : null;
		}
	},
	app: {
		/** Quits right away; call after checking for unsaved tabs. */
		quit: () => commands.appQuit(),
		version: () => getVersion(),
		/** Shows the app's log file in the file manager. */
		revealLog: async () => revealItemInDir(await unwrap(commands.appLogFile())),
		/** Opens a web page in the default browser. */
		openUrl: (url: string) => openUrl(url),
		/** Shows a file or folder in the file manager. */
		reveal: (path: string) => revealItemInDir(path),
		relaunch: () => relaunch()
	},
	updates: {
		/** The newer release, or null; fails in builds without the updater (dev, unsigned). */
		check: () => check()
	}
};

export function toApiError(err: unknown): ApiError {
	if (err instanceof ApiError) return err;
	return new ApiError({ kind: 'unknown', message: String(err) });
}

export * from './events.js';
