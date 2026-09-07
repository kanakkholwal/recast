/**
 * Review state for proposed branches. Reads the same host driver as
 * `agentSession`, so a host without a branch backend leaves this permanently
 * empty and the panel stays hidden.
 */

import type { EditorRenderState } from "../editor/render-state";
import type { ApplyReport, BranchSummary, FieldChange } from "./branches";
import { getAgentSessionDriver } from "./driver";
import type { AgentSessionEvent } from "./types";

function branchDriver() {
	return getAgentSessionDriver()?.branches ?? null;
}

function createBranchReview() {
	let projectPath = $state<string | null>(null);
	let branches = $state.raw<BranchSummary[]>([]);
	let selectedId = $state<string | null>(null);
	let changes = $state.raw<FieldChange[]>([]);
	let loading = $state(false);
	let busy = $state(false);
	let error = $state<string | null>(null);

	const available = $derived(branchDriver() !== null);
	const selected = $derived(branches.find((branch) => branch.id === selectedId) ?? null);

	function message(cause: unknown): string {
		if (typeof cause === "string") return cause;
		return cause instanceof Error ? cause.message : String(cause);
	}

	/** Run `job`, funnelling any rejection into `error` rather than throwing. */
	async function guard<T>(job: () => Promise<T>): Promise<T | null> {
		busy = true;
		error = null;
		try {
			return await job();
		} catch (cause) {
			error = message(cause);
			return null;
		} finally {
			busy = false;
		}
	}

	async function loadChanges(id: string) {
		const driver = branchDriver();
		const path = projectPath;
		if (!driver || !path) return;
		try {
			changes = await driver.diff(path, id);
		} catch (cause) {
			// A branch whose fork point moved can't be diffed, so the row stays selected and the reason sits beside it.
			changes = [];
			error = message(cause);
		}
	}

	async function refresh() {
		const driver = branchDriver();
		const path = projectPath;
		if (!driver || !path) return;
		loading = true;
		try {
			branches = await driver.list(path);
			if (selectedId && !branches.some((branch) => branch.id === selectedId)) {
				selectedId = null;
				changes = [];
			} else if (selectedId) {
				await loadChanges(selectedId);
			}
		} catch (cause) {
			error = message(cause);
		} finally {
			loading = false;
		}
	}

	return {
		get available() {
			return available;
		},
		get branches() {
			return branches;
		},
		get selected() {
			return selected;
		},
		get selectedId() {
			return selectedId;
		},
		get changes() {
			return changes;
		},
		get loading() {
			return loading;
		},
		get busy() {
			return busy;
		},
		get error() {
			return error;
		},
		get count() {
			return branches.length;
		},

		dismissError() {
			error = null;
		},

		async select(id: string | null) {
			selectedId = id;
			changes = [];
			if (id) await loadChanges(id);
		},

		refresh,

		/** Preview state a branch would produce, without applying it. */
		preview(id: string): Promise<Partial<EditorRenderState> | null> {
			const driver = branchDriver();
			const path = projectPath;
			if (!driver || !path) return Promise.resolve(null);
			return guard(() => driver.materialize(path, id));
		},

		async discard(id: string) {
			const driver = branchDriver();
			const path = projectPath;
			if (!driver || !path) return;
			await guard(() => driver.discard(path, id));
			await refresh();
		},

		async truncate(id: string, seq: number) {
			const driver = branchDriver();
			const path = projectPath;
			if (!driver || !path) return;
			await guard(() => driver.truncate(path, id, seq));
			await refresh();
		},

		/** Write a branch into the project. Returns null when it was rejected. */
		async apply(id: string, writerId: string): Promise<ApplyReport | null> {
			const driver = branchDriver();
			const path = projectPath;
			if (!driver || !path) return null;
			const report = await guard(() => driver.apply(path, id, writerId));
			await refresh();
			return report;
		},

		/** Subscribe for one project. Returns a cleanup fn for `$effect`. */
		bind(path: string): () => void {
			const driver = getAgentSessionDriver();
			projectPath = path;
			branches = [];
			selectedId = null;
			changes = [];
			error = null;
			if (!driver?.branches) return () => undefined;

			let disposed = false;
			let unsubscribe: (() => void) | undefined;

			const onEvent = (event: AgentSessionEvent) => {
				if (event.type !== "branches-changed") return;
				if (event.projectPath !== path) return;
				void refresh();
			};

			void (async () => {
				const off = await driver.subscribe(onEvent);
				if (disposed) off();
				else unsubscribe = off;
				await refresh();
			})();

			return () => {
				disposed = true;
				unsubscribe?.();
				projectPath = null;
				branches = [];
				selectedId = null;
				changes = [];
				error = null;
			};
		},
	};
}

export const branchReview = createBranchReview();
