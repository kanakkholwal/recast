/**
 * Export render worker (Phase 3): receives a serialized ExportJob, runs the
 * DOM-free consumer (run-export-job) — which owns its own OffscreenCanvas +
 * WebGL2 + MediaBunny loop — and transfers the encoded bytes back, so the whole
 * composite+encode runs off the main thread. Burned captions still route to the
 * main thread (font loading in a worker scope is a follow-up), so jobs reaching
 * here have `caption === null`.
 */

import { runExportJob } from "./run-export-job";
import type { FromExportWorker, ToExportWorker } from "./export-worker-protocol";

const post = (msg: FromExportWorker, transfer: Transferable[] = []) =>
	(self as unknown as Worker).postMessage(msg, transfer);

let aborter: AbortController | null = null;

self.onmessage = async (e: MessageEvent<ToExportWorker>) => {
	const msg = e.data;
	if (msg.type === "cancel") {
		aborter?.abort();
		return;
	}
	if (msg.type !== "render") return;
	aborter = new AbortController();
	try {
		const bytes = await runExportJob(msg.job, {
			onProgress: (fraction) => post({ type: "progress", fraction }),
			signal: aborter.signal,
		});
		post({ type: "done", bytes }, [bytes.buffer]);
	} catch (err) {
		post({ type: "error", message: err instanceof Error ? err.message : String(err) });
	}
};
