interface Pending {
	title: string;
	names: string[];
	/** What the user typed so far, starting from the values given last time. */
	values: Record<string, string>;
	resolve: (values: Record<string, string> | null) => void;
}

/**
 * Asks for several values at once, rendered by `<PromptValuesDialog>` in the root layout. Used
 * for `{{$prompt name}}` variables right before sending.
 */
class PromptValuesState {
	pending = $state<Pending>();

	ask(
		title: string,
		names: string[],
		initial: Record<string, string>
	): Promise<Record<string, string> | null> {
		this.pending?.resolve(null);
		return new Promise((resolve) => {
			this.pending = { title, names, values: { ...initial }, resolve };
		});
	}

	answer(values: Record<string, string> | null): void {
		this.pending?.resolve(values);
		this.pending = undefined;
	}
}

export const promptValues = new PromptValuesState();
