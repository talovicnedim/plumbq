interface PromptRequest {
	title: string;
	label: string;
	confirmLabel: string;
	initial?: string;
	placeholder?: string;
	/** Multi-line input, e.g. for a pasted cURL command. */
	multiline?: boolean;
}

interface Pending extends PromptRequest {
	resolve: (value: string | null) => void;
}

/** One text prompt at a time, rendered by `<PromptDialog>` in the root layout. */
class PromptState {
	pending = $state<Pending>();

	ask(request: PromptRequest): Promise<string | null> {
		this.pending?.resolve(null);
		return new Promise((resolve) => {
			this.pending = { ...request, resolve };
		});
	}

	answer(value: string | null): void {
		this.pending?.resolve(value);
		this.pending = undefined;
	}
}

export const promptState = new PromptState();

/** Asks for a line of text; resolves `null` on cancel. */
export function prompt(request: PromptRequest): Promise<string | null> {
	return promptState.ask(request);
}
