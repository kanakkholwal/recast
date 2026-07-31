/**
 * Browser export orchestrator (Phase 4c): snapshot the editor scene into a
 * serializable job (build-export-job), render + WebCodecs-encode it to an mp4
 * (run-export-job), persist it to a temp file, and return that path to hand to
 * the Rust mux job (`browserVideoPath`) — which copies the video (`-c:v copy`)
 * and adds the audio. One compositor, so preview and export can't diverge.
 *
 * The build/run split is the seam for moving the render off the main thread into
 * a worker (Phase 3): the producer stays main-thread (DOM asset prep), the
 * consumer is DOM-free and relocates unchanged.
 */

import { saveBrowserExportVideo } from "$lib/ipc";
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import type { ExportQuality } from "./browser-export-plan";
import { buildExportJob, type ExportJobInputs } from "./build-export-job";
import { runExportJob, type ExportRuntime } from "./run-export-job";
import { exportWorkerSupported, runExportJobInWorker } from "./export-worker-client";

export interface BrowserExportOptions {
	/** Source video asset URL (what the preview decodes, e.g. `convertFileSrc(...)`). */
	videoUrl: string;
	/** Camera stream URL (`convertFileSrc(camera.mp4)`), or empty when none. */
	cameraUrl?: string;
	quality: ExportQuality;
	fps: number;
	onProgress?: (fraction: number) => void;
	signal?: AbortSignal;
}

/** Render + encode the timeline in the browser; resolves with the temp path of
 *  the video-only mp4 to mux server-side. Throws if the source isn't ready. */
export async function runBrowserExport(
	store: EditorStore,
	opts: BrowserExportOptions,
): Promise<string> {
	const jobOpts: ExportJobInputs = {
		videoUrl: opts.videoUrl,
		cameraUrl: opts.cameraUrl,
		quality: opts.quality,
		fps: opts.fps,
	};
	const mp4 = await renderToBytes(store, jobOpts, {
		onProgress: opts.onProgress,
		signal: opts.signal,
	});
	// Copy out of the (possibly larger) backing buffer so the transfer is exact.
	const bytes = mp4.buffer.slice(mp4.byteOffset, mp4.byteOffset + mp4.byteLength) as ArrayBuffer;
	return await saveBrowserExportVideo(bytes);
}

/** Render off the main thread when possible, else on it. Burned captions still
 *  render main-thread (worker font loading is a follow-up). On any worker failure
 *  the transferred bitmaps are gone, so a fresh job is rebuilt for the fallback. */
async function renderToBytes(
	store: EditorStore,
	jobOpts: ExportJobInputs,
	runtime: ExportRuntime,
): Promise<Uint8Array> {
	const job = await buildExportJob(store, jobOpts);
	if (exportWorkerSupported() && !job.caption) {
		try {
			return await runExportJobInWorker(job, runtime);
		} catch (err) {
			if (runtime.signal?.aborted) throw err;
			console.warn("export worker failed; falling back to main-thread render", err);
			const fresh = await buildExportJob(store, jobOpts);
			return runExportJob(fresh, runtime);
		}
	}
	return runExportJob(job, runtime);
}
