/**
 * Main-thread handle to the export render worker: posts a job (bitmaps
 * transferred zero-copy), relays progress, forwards cancellation, and resolves
 * with the encoded bytes. One worker per export — terminated on completion.
 */

import { collectTransferables, type ExportJob } from "./export-job";
import type { ExportRuntime } from "./run-export-job";
import type { FromExportWorker, ToExportWorker } from "./export-worker-protocol";

/** Whether this runtime can host the export worker (dedicated Worker + OffscreenCanvas). */
export function exportWorkerSupported(): boolean {
	return typeof Worker !== "undefined" && typeof OffscreenCanvas !== "undefined";
}

/** Render a job in a worker → encoded mp4 bytes. Rejects on worker error (the
 *  caller rebuilds a fresh job for the main-thread fallback, since the bitmaps
 *  were transferred out). Rejects with the abort reason if cancelled. */
export function runExportJobInWorker(
	job: ExportJob,
	runtime: ExportRuntime = {},
): Promise<Uint8Array> {
	return new Promise<Uint8Array>((resolve, reject) => {
		if (runtime.signal?.aborted) {
			reject(new DOMException("export cancelled", "AbortError"));
			return;
		}
		const worker = new Worker(new URL("./export-render.worker.ts", import.meta.url), {
			type: "module",
		});
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
		post({ type: "render", job }, collectTransferables(job));
	});
}
