import { browser } from "$app/environment";
import { enqueueExport, type RunExportOptions } from "$lib/services/export";
import { notifyJobDone } from "$lib/notify";
import {
	type ExportJobDto,
	type ExportStateEvent,
	cancelExportJob,
	dismissExportJob,
	listExportJobs,
	listenToAllExportState,
	listenToExportJobsChanged,
	refreshTray,
	retryExportJob,
} from "$lib/ipc";
import {
	clearJobProgress,
	setJobProgress,
	setJobProgressIndeterminate,
} from "$lib/taskbarProgress";
import { toast } from "@recast/ui/sonner";

/**
 * Export activity store: a `$state`-backed singleton that mirrors the
 * BACKEND-owned export queue (`commands::export_queue`). The backend is the single
 * source of truth: it persists, runs (one at a time), and reports every export.
 * This store is a READ-MODEL over that, driven by two event streams:
 *
 *  - `export-jobs-changed` -> re-fetch `list_export_jobs` (membership + status)
 *  - `export-state`        -> live progress/phase for the running job, and the
 *                             one-shot success/cancel/error user feedback
 *
 * The editor builds a self-contained payload in the browser (render state is
 * rasterized there) and hands it off via {@link enqueue}; from then on the job
 * lives in the backend and survives closing its editor or restarting the app.
 * `show`/`minimize`/`foreground` are purely local UI state (which job the editor
 * panel is showing). Mirrors the cloudShare / gdrive upload stores.
 */

export type ExportItemStatus =
	| "queued"
	| "running"
	| "success"
	| "error"
	| "cancelled"
	| "interrupted";

export type ExportItemPhase = "preparing" | "encoding" | "finalizing" | "cancelling";

/** Everything needed to run an export, captured at enqueue time (render state
 *  included) so the backend can run it after the source editor is closed. */
export type ExportRunParams = Omit<RunExportOptions, "exportId" | "onState">;

export interface ExportItem {
	id: string;
	filename: string;
	/** Source project path, for display + same-project panel adoption. */
	filePath: string;
	status: ExportItemStatus;
	phase: ExportItemPhase;
	/** 0..100. Held at 100 on success. */
	progress: number;
	/** When the FFmpeg run started (null while queued), for the ETA readout. */
	startedAt: number | null;
	/** Output path once it succeeds. */
	path?: string;
	/** Failure message once it errors. */
	error?: string;
}

function messageOf(e: unknown): string {
	return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

function baseName(path: string): string {
	return path.split(/[\\/]/).pop() ?? path;
}

function fromDto(d: ExportJobDto): ExportItem {
	return {
		id: d.id,
		filename: d.filename,
		filePath: d.filePath,
		status: d.status,
		phase: d.phase,
		progress: d.progress,
		startedAt: d.startedAt ?? null,
		path: d.path ?? undefined,
		error: d.error ?? undefined,
	};
}

function createExportActivityStore() {
	// The queue as the backend reports it (queued + running + undismissed
	// terminal), oldest first. Mutated in place (splice/push) so the array
	// identity stays stable for Svelte reactivity.
	const items = $state<ExportItem[]>([]);
	// Whether the editor's export panel is shown. Minimizing hands tracking to the
	// activity center; reopening from there (or the toolbar) sets it back.
	let foreground = $state(false);
	// The item id the editor panel is currently showing (null during the options
	// picker), so the activity center can hide just that one to avoid doubling.
	let foregroundId = $state<string | null>(null);
	// Whether an editor (which hosts the export panel) is mounted, so the activity
	// center knows a "foregrounded" job actually has a panel on screen.
	let editorPresent = $state(false);

	const find = (id: string) => items.find((i) => i.id === id);
	const runningItem = () => items.find((i) => i.status === "running");

	/** Reconcile the local list with the backend, preserving the live progress of a
	 *  still-running job (the DB snapshot is coarse; `export-state` carries the
	 *  smooth value). */
	async function refreshList() {
		let rows: ExportJobDto[];
		try {
			rows = await listExportJobs();
		} catch (e) {
			console.warn("[exportActivity] list failed", e);
			return;
		}
		const prev = new Map(items.map((i) => [i.id, i]));
		const next = rows.map((d) => {
			const item = fromDto(d);
			const p = prev.get(d.id);
			if (item.status === "running" && p && p.status === "running") {
				item.progress = Math.max(item.progress, p.progress);
				item.phase = p.phase;
			}
			return item;
		});
		items.splice(0, items.length, ...next);
	}

	/** One-shot user feedback + taskbar clear on a terminal outcome. Fired from the
	 *  `export-state` stream (the live signal) so it happens exactly once. */
	function finishFeedback(
		it: ExportItem,
		status: "success" | "cancelled" | "error",
		path?: string,
		error?: string,
	) {
		it.status = status;
		if (status === "success") {
			it.progress = 100;
			it.path = path;
		} else if (status === "error") {
			it.error = error;
		}
		void clearJobProgress();
		if (status === "success") {
			toast.success("Export complete", { description: it.filename });
			void notifyJobDone("Export complete", baseName(path ?? it.filename));
			void refreshTray(null).catch(() => {});
		} else if (status === "cancelled") {
			toast.info("Export cancelled");
		} else {
			toast.error("Export failed");
		}
	}

	/** Live progress/phase + terminal feedback from the Rust pipeline. */
	function applyState(e: ExportStateEvent) {
		const it = find(e.exportId);
		if (!it) return;
		switch (e.status) {
			case "started":
			case "preparing":
				if (it.status === "running") it.phase = "preparing";
				break;
			case "progress": {
				if (it.status !== "running") return;
				const next = Math.min(100, Math.max(0, e.progress));
				if (it.phase === "preparing") it.phase = "encoding";
				it.progress = Math.max(it.progress, next);
				void setJobProgress(it.progress);
				break;
			}
			case "finalizing":
				if (it.status === "running") {
					it.phase = "finalizing";
					void setJobProgressIndeterminate();
				}
				break;
			case "success":
				finishFeedback(it, "success", e.path);
				break;
			case "cancelled":
				finishFeedback(it, "cancelled");
				break;
			case "error":
				finishFeedback(it, "error", undefined, e.message);
				break;
		}
	}

	// Module-singleton wiring: hydrate once and keep the read-model live. Guarded to
	// the browser so importing this during SSR/prerender doesn't touch Tauri.
	let initialized = false;
	function ensureInit() {
		if (initialized || !browser) return;
		initialized = true;
		void refreshList();
		void listenToExportJobsChanged(() => void refreshList());
		void listenToAllExportState((e) => applyState(e));
	}
	ensureInit();

	return {
		get items() {
			return items;
		},
		/** Any export currently encoding. */
		get running() {
			return runningItem() != null;
		},
		/** Any item at all (queued, running, or an undismissed result). */
		get active() {
			return items.length > 0;
		},
		get foreground() {
			return foreground;
		},
		get foregroundId() {
			return foregroundId;
		},
		get editorPresent() {
			return editorPresent;
		},

		item(id: string): ExportItem | null {
			return find(id) ?? null;
		},
		/** 1-based position of a queued item behind the running one (0 if running). */
		queuePosition(id: string): number {
			const active = items.filter((i) => i.status === "queued" || i.status === "running");
			return Math.max(
				0,
				active.findIndex((i) => i.id === id),
			);
		},

		/** Editor mount lifecycle sets this (see the activity center hide rule). */
		setEditorPresent(present: boolean) {
			editorPresent = present;
		},
		/** Show the export panel in the editor for a given item (null = options). */
		show(id: string | null = null) {
			foreground = true;
			foregroundId = id;
		},
		/** Hide the panel and keep tracking in the activity center. */
		minimize() {
			foreground = false;
		},

		/** Hand a fully-built export to the backend queue. Inserts an optimistic
		 *  `queued` item so the editor's ring/panel show immediately, then reconciles
		 *  with the backend on the next `export-jobs-changed`. */
		enqueue(spec: { id: string; filename: string; filePath: string; params: ExportRunParams }) {
			if (!find(spec.id)) {
				items.push({
					id: spec.id,
					filename: spec.filename,
					filePath: spec.filePath,
					status: "queued",
					phase: "preparing",
					progress: 0,
					startedAt: null,
				});
			}
			void enqueueExport({ ...spec.params, exportId: spec.id })
				.then((repairs) => {
					// The backend clamped a stale/out-of-range render state (e.g. a
					// too-long trim_end from an older recording) so the export could run.
					// Surface it so the result gets a manual sanity check.
					if (repairs.length > 0) {
						toast.warning("Auto-repaired the timeline before export", {
							description: `${repairs.join("; ")}. Please double-check the exported video.`,
						});
					}
				})
				.catch((e) => {
					const idx = items.findIndex((i) => i.id === spec.id);
					if (idx >= 0) items.splice(idx, 1);
					toast.error("Couldn't queue the export", {
						description: messageOf(e),
					});
					console.error("[exportActivity] enqueue failed", e);
				});
		},

		/** Cancel/remove an item: a queued one is dropped; a running one is stopped.
		 *  Optimistic locally, then reconciled via `export-jobs-changed`. */
		async cancel(id: string) {
			const it = find(id);
			if (it) {
				if (it.status === "queued") {
					const idx = items.indexOf(it);
					if (idx >= 0) items.splice(idx, 1);
				} else if (it.status === "running") {
					it.phase = "cancelling";
				}
			}
			try {
				await cancelExportJob(id);
			} catch (e) {
				// The optimistic change may now disagree with the backend; resync.
				console.warn("[exportActivity] cancel failed", e);
				void refreshList();
			}
		},

		/** Remove a finished (non-running) item from the list. */
		dismiss(id: string) {
			const idx = items.findIndex((i) => i.id === id);
			if (idx >= 0 && items[idx].status !== "running") items.splice(idx, 1);
			void dismissExportJob(id).catch((e) => {
				console.warn("[exportActivity] dismiss failed", e);
				void refreshList();
			});
		},

		/** Requeue a failed/cancelled/interrupted item (payload is still on disk). */
		retry(id: string) {
			const it = find(id);
			if (it) {
				it.status = "queued";
				it.phase = "preparing";
				it.progress = 0;
				it.startedAt = null;
				it.path = undefined;
				it.error = undefined;
			}
			void retryExportJob(id).catch((e) => {
				console.warn("[exportActivity] retry failed", e);
				void refreshList();
			});
		},
	};
}

export const exportActivity = createExportActivityStore();
