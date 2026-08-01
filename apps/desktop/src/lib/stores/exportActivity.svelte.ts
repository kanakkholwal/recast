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
	saveBrowserExportVideo,
} from "$lib/ipc";
import { renderJobToBytes } from "$lib/export/browser-export";
import type { ExportJob } from "$lib/export/export-job";
import {
	clearJobProgress,
	setJobProgress,
	setJobProgressIndeterminate,
} from "$lib/taskbarProgress";
import { log } from "$lib/logger";
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

export type ExportItemPhase = "preparing" | "rendering" | "encoding" | "finalizing" | "cancelling";

/** The render phase (browser compositing) owns 0..RENDER_MAX of the unified bar;
 *  the backend mux is the fast tail from RENDER_MAX..100. */
const RENDER_MAX = 95;

/** Everything needed to run an export, captured at enqueue time (render state
 *  included) so the backend can run it after the source editor is closed. */
export type ExportRunParams = Omit<RunExportOptions, "exportId" | "onState">;

/** Snapshot for the `export_completed` performance event: correlate wall time
 *  against the source (duration/resolution/fps/size) + settings. Captured at
 *  enqueue since the source editor may be gone by the time the export finishes. */
export interface ExportTelemetry {
	engine: "browser" | "rust";
	format: string;
	quality: string;
	/** Output length after cuts/speed (seconds) — what actually gets rendered. */
	outputDurationSec: number;
	srcDurationSec: number;
	srcWidth: number;
	srcHeight: number;
	srcFps: number;
	srcCodec: string;
	srcBytes: number;
}

export interface ExportItem {
	id: string;
	filename: string;
	/** Source project path, for display + same-project panel adoption. */
	filePath: string;
	status: ExportItemStatus;
	phase: ExportItemPhase;
	/** 0..100. Held at 100 on success. */
	progress: number;
	/** When the FFmpeg run started (null while queued), for the ETA readout. Wall
	 *  clock (Date.now / backend unix-ms) so it survives the DTO round-trip. */
	startedAt: number | null;
	/** Monotonic (performance.now) start, for accurate duration telemetry — immune
	 *  to clock adjustments, unlike the wall-clock `startedAt`. Local-only. */
	perfStartedAt?: number;
	/** When it reached a terminal state, for the "Exported in …" readout. */
	finishedAt?: number;
	/** Output path once it succeeds. */
	path?: string;
	/** Failure message once it errors. */
	error?: string;
	/** True for browser-engine exports: a render phase (0..RENDER_MAX) precedes the
	 *  backend mux, so backend progress maps to the RENDER_MAX..100 tail. */
	hasRenderPhase?: boolean;
	/** Performance snapshot, emitted on finish. Local-only (not in the DTO). */
	telemetry?: ExportTelemetry;
	/** Browser render wall time (ms) + the video-only byte size it produced. */
	renderMs?: number;
	outputBytes?: number;
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
		finishedAt: d.finishedAt ?? undefined,
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

	// Browser-render phase: pending render jobs + the in-flight one. The render runs
	// HERE (app-scoped) so it survives closing its editor, serial (N=1) so two
	// encoders never contend. The rendered video is then handed to the backend queue.
	type RenderReq = {
		id: string;
		job: ExportJob;
		spec: { filename: string; filePath: string; params: ExportRunParams };
	};
	const renderQueue: RenderReq[] = [];
	let renderingId: string | null = null;
	let renderAbort: AbortController | null = null;
	let renderRunning = false;

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
		const backendIds = new Set(rows.map((d) => d.id));
		const next = rows.map((d) => {
			const item = fromDto(d);
			const p = prev.get(d.id);
			if (item.status === "running" && p && p.status === "running") {
				item.progress = Math.max(item.progress, p.progress);
				item.phase = p.phase;
			}
			// hasRenderPhase is local-only (not in the DTO); carry it across so the
			// backend mux keeps mapping onto the RENDER_MAX..100 tail.
			if (p?.hasRenderPhase) {
				item.hasRenderPhase = true;
				// Keep the render's start (not the backend mux's) so total time + ETA
				// span the whole render→mux, not just the fast tail.
				if (p.startedAt) item.startedAt = p.startedAt;
			}
			// Carry the other local-only fields so the finish telemetry stays complete.
			if (p?.telemetry) item.telemetry = p.telemetry;
			if (p?.perfStartedAt != null) item.perfStartedAt = p.perfStartedAt;
			if (p?.renderMs != null) item.renderMs = p.renderMs;
			if (p?.outputBytes != null) item.outputBytes = p.outputBytes;
			return item;
		});
		// Keep local items still in the browser-render phase — the backend doesn't
		// know about them until the render hands off, so they're absent from `rows`.
		const localRenders = items.filter(
			(i) =>
				i.hasRenderPhase &&
				!backendIds.has(i.id) &&
				(i.status === "running" || i.status === "queued"),
		);
		items.splice(0, items.length, ...next, ...localRenders);
	}

	/** Performance event on every terminal outcome: wall time vs source metrics,
	 *  so we can see how long exports take for a given length/resolution/size. */
	function emitCompletedTelemetry(it: ExportItem, status: "success" | "cancelled" | "error") {
		// Browser render is timed monotonically (clock-safe, exact render start); the
		// Rust path prefers the backend's authoritative wall span (SQLite start→finish).
		const monotonicMs =
			it.perfStartedAt != null ? Math.round(performance.now() - it.perfStartedAt) : undefined;
		const wallMs =
			it.startedAt != null && it.finishedAt != null ? it.finishedAt - it.startedAt : undefined;
		const totalMs = it.hasRenderPhase ? (monotonicMs ?? wallMs) : (wallMs ?? monotonicMs);
		const t = it.telemetry;
		log.info("export", "export_completed", {
			exportId: it.id,
			status,
			engine: t?.engine,
			format: t?.format,
			quality: t?.quality,
			totalMs,
			renderMs: it.renderMs,
			outputBytes: it.outputBytes,
			outputDurationSec: t ? Math.round(t.outputDurationSec) : undefined,
			srcDurationSec: t ? Math.round(t.srcDurationSec) : undefined,
			srcWidth: t?.srcWidth,
			srcHeight: t?.srcHeight,
			srcFps: t?.srcFps,
			srcCodec: t?.srcCodec,
			srcBytes: t?.srcBytes,
		});
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
		it.finishedAt = Date.now();
		if (status === "success") {
			it.progress = 100;
			it.path = path;
		} else if (status === "error") {
			it.error = error;
		}
		emitCompletedTelemetry(it, status);
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
		// First live signal of processing → stamp the monotonic start (browser items
		// already stamped it at render start; this covers the Rust path).
		if (
			it.perfStartedAt == null &&
			(e.status === "started" || e.status === "preparing" || e.status === "progress")
		) {
			it.perfStartedAt = performance.now();
		}
		switch (e.status) {
			case "started":
			case "preparing":
				// Browser items already rendered 0..RENDER_MAX; the backend is only the mux.
				if (it.status === "running") it.phase = it.hasRenderPhase ? "finalizing" : "preparing";
				break;
			case "progress": {
				if (it.status !== "running") return;
				if (it.hasRenderPhase) {
					// Map the backend mux (0..100) onto the RENDER_MAX..100 tail of the bar.
					it.phase = "finalizing";
					const frac = Math.min(100, Math.max(0, e.progress)) / 100;
					it.progress = Math.max(it.progress, RENDER_MAX + Math.round(frac * (100 - RENDER_MAX)));
					void setJobProgress(it.progress);
					return;
				}
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

	/** Serial render runner: composite the next queued job in the browser (off the
	 *  main thread), persist the video, then hand it to the backend queue, which
	 *  drives the mux tail. One render at a time; loops until the queue drains. */
	async function pumpRenderQueue() {
		if (renderRunning) return;
		const req = renderQueue.shift();
		if (!req) return;
		renderRunning = true;
		renderingId = req.id;
		renderAbort = new AbortController();
		const perfStart = performance.now();
		const it = find(req.id);
		if (it) {
			it.status = "running";
			it.phase = "rendering";
			it.startedAt = Date.now();
			it.perfStartedAt = perfStart;
			it.progress = 0;
		}
		try {
			const bytes = await renderJobToBytes(req.job, {
				signal: renderAbort.signal,
				onProgress: (f) => {
					const item = find(req.id);
					if (item && item.status === "running" && item.phase === "rendering") {
						item.progress = Math.min(RENDER_MAX, Math.round(f * RENDER_MAX));
						void setJobProgress(item.progress);
					}
				},
			});
			const rendered = find(req.id);
			if (rendered) {
				rendered.renderMs = Math.round(performance.now() - perfStart);
				rendered.outputBytes = bytes.byteLength;
			}
			const exact = bytes.buffer.slice(
				bytes.byteOffset,
				bytes.byteOffset + bytes.byteLength,
			) as ArrayBuffer;
			const browserVideoPath = await saveBrowserExportVideo(exact);
			// Hand off to the durable backend queue; its events drive the mux tail.
			void enqueueExport({ ...req.spec.params, browserVideoPath, exportId: req.id })
				.then((repairs) => {
					if (repairs.length > 0) {
						toast.warning("Auto-repaired the timeline before export", {
							description: `${repairs.join("; ")}. Please double-check the exported video.`,
						});
					}
				})
				.catch((e) => {
					const item = find(req.id);
					if (item) finishFeedback(item, "error", undefined, messageOf(e));
				});
		} catch (err) {
			const item = find(req.id);
			if (item) {
				if (renderAbort?.signal.aborted) finishFeedback(item, "cancelled");
				else finishFeedback(item, "error", undefined, messageOf(err));
			}
		} finally {
			renderRunning = false;
			renderingId = null;
			renderAbort = null;
			void pumpRenderQueue();
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
		enqueue(spec: {
			id: string;
			filename: string;
			filePath: string;
			params: ExportRunParams;
			telemetry?: ExportTelemetry;
		}) {
			if (!find(spec.id)) {
				items.push({
					id: spec.id,
					filename: spec.filename,
					filePath: spec.filePath,
					status: "queued",
					phase: "preparing",
					progress: 0,
					startedAt: null,
					telemetry: spec.telemetry,
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

		/** Hand a browser-engine export to the app-scoped render queue. The render
		 *  runs here (surviving the source editor closing), then feeds the backend
		 *  queue via {@link enqueue}. `job` is the pre-built self-contained snapshot. */
		enqueueBrowserExport(spec: {
			id: string;
			filename: string;
			filePath: string;
			job: ExportJob;
			params: ExportRunParams;
			telemetry?: ExportTelemetry;
		}) {
			if (!find(spec.id)) {
				items.push({
					id: spec.id,
					filename: spec.filename,
					filePath: spec.filePath,
					status: "queued",
					phase: "rendering",
					progress: 0,
					startedAt: null,
					hasRenderPhase: true,
					telemetry: spec.telemetry,
				});
			}
			renderQueue.push({
				id: spec.id,
				job: spec.job,
				spec: { filename: spec.filename, filePath: spec.filePath, params: spec.params },
			});
			void pumpRenderQueue();
		},

		/** Cancel/remove an item: a queued one is dropped; a running one is stopped.
		 *  Optimistic locally, then reconciled via `export-jobs-changed`. */
		async cancel(id: string) {
			// Browser-render phase: abort the in-flight render, or drop a queued one —
			// the backend has no job for it yet, so there's nothing to cancel there.
			if (id === renderingId) {
				renderAbort?.abort();
				return;
			}
			const qi = renderQueue.findIndex((r) => r.id === id);
			if (qi >= 0) {
				renderQueue.splice(qi, 1);
				const idx = items.findIndex((i) => i.id === id);
				if (idx >= 0) items.splice(idx, 1);
				return;
			}
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
