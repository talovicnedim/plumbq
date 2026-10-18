import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';

type MenuAction = 'open-workspace' | 'new-request' | 'close-tab' | 'settings';

/** Subscribes to native menu actions (macOS menu bar). Returns an unsubscribe function. */
export function onMenu(handler: (action: MenuAction) => void): () => void {
	const unlisten = listen<MenuAction>('menu', (event) => handler(event.payload));
	return () => void unlisten.then((stop) => stop());
}

/** Subscribes to quit requests (⌘Q, closing the window). Returns an unsubscribe function. */
export function onQuitRequested(handler: () => void): () => void {
	const unlisten = listen('quit-requested', () => handler());
	return () => void unlisten.then((stop) => stop());
}

/** Subscribes to changes in the AI requests waiting for approval. Returns an unsubscribe function. */
export function onApproval(handler: () => void): () => void {
	const unlisten = listen('mcp-approval', () => handler());
	return () => void unlisten.then((stop) => stop());
}

/** Subscribes to file changes in the open workspace. Returns an unsubscribe function. */
export function onWorkspaceChanged(handler: () => void): () => void {
	const unlisten = listen('workspace-changed', () => handler());
	return () => void unlisten.then((stop) => stop());
}

/** Paths of files dragged onto the window: entering, leaving or dropped. */
export type FileDrag =
	{ kind: 'enter'; paths: string[] } | { kind: 'leave' } | { kind: 'drop'; paths: string[] };

/** Subscribes to files dragged onto the window. Returns an unsubscribe function. */
export function onFileDrag(handler: (drag: FileDrag) => void): () => void {
	const unlisten = getCurrentWebview().onDragDropEvent(({ payload }) => {
		if (payload.type === 'enter') handler({ kind: 'enter', paths: payload.paths });
		if (payload.type === 'leave') handler({ kind: 'leave' });
		if (payload.type === 'drop') handler({ kind: 'drop', paths: payload.paths });
	});
	return () => void unlisten.then((stop) => stop());
}

/** Subscribes to the window's close button; the window stays open. Returns an unsubscribe function. */
export function onCloseRequested(handler: () => void): () => void {
	const unlisten = getCurrentWindow().onCloseRequested((event) => {
		event.preventDefault();
		handler();
	});
	return () => void unlisten.then((stop) => stop());
}
