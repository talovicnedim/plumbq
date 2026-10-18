import { toast } from 'svelte-sonner';
import {
	api,
	toApiError,
	type RunOutcome,
	type RunPreview,
	type RunReport
} from '#lib/api/index.js';
import { addOutcome, emptyResults, parseTags, type RunResults } from '#lib/utils/runner.js';
import { confirm } from './confirm.svelte.js';
import { environments } from './environments.svelte.js';
import { promptValues } from './prompt-values.svelte.js';
import { tabs } from './tabs.svelte.js';

/** The Runner panel: what to run, the live results, and the finished report. */
class RunnerState {
	open = $state(false);
	/** Folder path, request id, or `""` for the whole workspace. */
	target = $state('');
	tags = $state('');
	delayMs = $state(0);
	iterations = $state(1);
	dataFile = $state<string | null>(null);
	bail = $state(false);
	running = $state(false);
	/** What core says the run would send and need; refreshed when the panel opens or tags change. */
	preview = $state<RunPreview>();
	/** Live while running, then the report's: exact counts, a capped list. */
	results = $state<RunResults>(emptyResults());
	report = $state<RunReport>();
	/** Last `{{$prompt}}` answers, to prefill the next run. Memory only, never saved. */
	#answers: Record<string, string> = {};

	show(target: string): void {
		this.target = target;
		this.results = emptyResults();
		this.report = undefined;
		this.open = true;
		void this.refreshPreview();
	}

	async refreshPreview(): Promise<void> {
		try {
			this.preview = await this.#preview();
		} catch (err) {
			this.preview = undefined;
			toast.error(toApiError(err).message);
		}
	}

	async chooseData(): Promise<void> {
		const path = await api.dialog.open('dataFile');
		if (path) this.dataFile = path;
	}

	async start(): Promise<void> {
		if (this.running) return;
		let preview: RunPreview;
		try {
			preview = await this.#preview();
		} catch (err) {
			toast.error(toApiError(err).message);
			return;
		}
		this.preview = preview;
		const prompts = await this.#askPrompts(preview.prompts);
		if (!prompts) return;
		const writes = preview.productionWrites > 0;
		if (writes && !(await this.#confirmProductionWrites(preview.productionWrites))) return;
		await this.#run(prompts, writes, preview.keepOutcomes);
	}

	async #run(
		prompts: Record<string, string>,
		allowProductionWrites: boolean,
		keep: number
	): Promise<void> {
		this.results = emptyResults();
		this.report = undefined;
		this.running = true;
		// Outcomes can arrive after the report, which already holds them.
		let ended = false;
		const onOutcome = (outcome: RunOutcome) => {
			if (!ended) addOutcome(this.results, outcome, keep);
		};
		try {
			const report = await api.runner
				.run(
					{
						target: this.target,
						environment: environments.selected,
						bail: this.bail,
						tags: parseTags(this.tags),
						delayMs: this.delayMs,
						iterations: Math.max(1, this.iterations),
						dataFile: this.dataFile,
						prompts,
						allowProductionWrites
					},
					onOutcome
				)
				.finally(() => (ended = true));
			this.report = report;
			this.results = report;
		} catch (err) {
			toast.error(toApiError(err).message);
		} finally {
			this.running = false;
		}
	}

	#preview(): Promise<RunPreview> {
		return api.runner.preview(this.target, parseTags(this.tags), environments.selected);
	}

	/** Asks once for every `{{$prompt}}` value the run needs; `null` when cancelled. */
	async #askPrompts(names: string[]): Promise<Record<string, string> | null> {
		if (names.length === 0) return {};
		const answers = await promptValues.ask('Values for this run', names, this.#answers);
		if (answers) this.#answers = answers;
		return answers;
	}

	#confirmProductionWrites(count: number): Promise<boolean> {
		const env = environments.selected ?? '';
		return confirm({
			title: 'Send writes to production?',
			description: `${count} request${count === 1 ? '' : 's'} in this run would change data in "${env}".`,
			confirmLabel: 'Send them',
			destructive: true,
			typeToConfirm: env
		});
	}

	stop(): void {
		void api.runner.stop();
	}

	/** Opens a result's request with the response it got in this run. */
	async showOutcome(outcome: RunOutcome): Promise<void> {
		await tabs.open(outcome.id);
		const tab = tabs.all.find((t) => t.id === outcome.id);
		if (tab && outcome.historyId) {
			try {
				tab.entry = await api.history.get(outcome.id, outcome.historyId);
				tab.error = undefined;
			} catch (err) {
				toast.error(toApiError(err).message);
			}
		}
		this.open = false;
	}

	async exportReport(): Promise<void> {
		if (!this.report) return;
		try {
			const path = await api.runner.exportReport(this.report, 'results.xml');
			if (path) toast.success('Results saved', { description: path });
		} catch (err) {
			toast.error(toApiError(err).message);
		}
	}
}

export const runner = new RunnerState();

/** Saves a folder (`""` for everything) as a Postman collection plus environment files. */
export async function exportPostman(folder: string): Promise<void> {
	const name = folder.split('/').pop() || 'collection';
	try {
		const written = await api.export.postman(folder, `${name}.postman_collection.json`);
		if (!written) return;
		const envs = written.length - 1;
		toast.success('Exported to Postman v2.1', {
			description:
				envs > 0 ? `Plus ${envs} environment file${envs === 1 ? '' : 's'} next to it.` : undefined
		});
	} catch (err) {
		toast.error(toApiError(err).message);
	}
}

/** Saves a single-page HTML documentation of the workspace. */
export async function exportDocs(): Promise<void> {
	try {
		const path = await api.export.docs(environments.selected, 'api-docs.html');
		if (!path) return;
		toast.success('Docs generated', {
			description: environments.selected
				? `With the variables of ${environments.selected}.`
				: undefined
		});
	} catch (err) {
		toast.error(toApiError(err).message);
	}
}
