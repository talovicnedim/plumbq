import { toast } from 'svelte-sonner';
import { api, toApiError, type MultipartField } from '#lib/api/index.js';

/**
 * A multipart body as fields for the editor. Core reads and writes the body; the fields the
 * user types into change at once, so typing never waits for it.
 */
export class MultipartFields {
	/** Undefined until core has read the body. */
	fields = $state<MultipartField[]>();
	/** The body written last from these fields, so it isn't read back over them. */
	#written: string | undefined;
	/** Only core's newest answer counts. */
	#ticket = 0;

	/** Reads `body` when it changed from outside: a reload, or an edit in another view. */
	read(body: string, boundary: string): void {
		if (body === this.#written) return;
		const ticket = ++this.#ticket;
		api.multipart.parse(body, boundary).then((fields) => {
			if (ticket === this.#ticket) this.fields = fields;
		}, showError);
	}

	/** Shows `fields` now and hands the body core writes for them to `onbody`. */
	write(fields: MultipartField[], boundary: string, onbody: (body: string) => void): void {
		this.fields = fields;
		const ticket = ++this.#ticket;
		api.multipart.format(fields, boundary).then((body) => {
			if (ticket !== this.#ticket) return;
			this.#written = body;
			onbody(body);
		}, showError);
	}
}

function showError(err: unknown): void {
	toast.error(toApiError(err).message);
}
