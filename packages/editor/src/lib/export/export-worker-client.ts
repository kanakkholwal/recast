/**
 * Main-thread handle to the export render worker: posts a job (bitmaps
 * transferred zero-copy), relays progress, forwards cancellation, and resolves
 * with the encoded bytes. One worker per export — terminated on completion.
 */

import { createEditorWorker } from "../host-hooks";
import { collectTransferables, type ExportJob } from "./export-job";
import type { FromExportWorker, ToExportWorker } from "./export-worker-protocol";
import type { ExportRuntime } from "./run-export-job";

/** Whether this runtime can host the export worker (dedicated Worker + OffscreenCanvas). */
export function exportWorkerSupported(): boolean {
	return typeof Worker !== "undefined" && typeof OffscreenCanvas !== "undefined";
}

/** Render a job in a worker → encoded mp4 bytes. With `transfer` (default) the
 *  bitmaps move zero-copy but the job is left detached; pass `transfer: false` to
 *  clone them instead, keeping the job re-runnable for a main-thread fallback
 *  when the store isn't around to rebuild it. Rejects on worker error / cancel. */
export function runExportJobInWorker(
	job: ExportJob,
	runtime: ExportRuntime = {},
	opts: { transfer?: boolean } = {},
): Promise<Uint8Array> {
	return new Promise<Uint8Array>((resolve, reject) => {
		if (runtime.signal?.aborted) {
			reject(new DOMException("export cancelled", "AbortError"));
			return;
		}
		const worker = createEditorWorker("exportRender");
		const onAbort = () => post({ type: "cancel" });
		const cleanup = () => {
			runtime.signal?.removeEventListener("abort", onAbort);
			worker.terminate();
		};
		const post = (m: ToExportWorker, transfer: Transferable[] = []) =>
			worker.postMessage(m, transfer);

		worker.onmessage = (e: MessageEvent<FromExportWorker>) => {
			const m = e.data;
			if (m.type === "progress") runtime.onProgress?.(m.fraction);
			else if (m.type === "done") {
				cleanup();
				resolve(m.bytes);
			} else if (m.type === "error") {
				cleanup();
				reject(new Error(m.message));
			}
		};
		worker.onerror = (e) => {
			cleanup();
			reject(new Error(e.message || "export worker crashed"));
		};

		runtime.signal?.addEventListener("abort", onAbort);
		post({ type: "render", job }, opts.transfer === false ? [] : collectTransferables(job));
	});
}
