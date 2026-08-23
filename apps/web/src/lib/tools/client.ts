/**
 * Main-thread entry point for the browser conversion tools. UI calls
 * `runConversion(file, op, options, { onProgress, signal })` and gets back a
 * Blob; everything else (worker lifecycle, job ids, progress fan-out,
 * cancellation) is handled here. Nothing is uploaded — the worker runs in the
 * user's browser.
 */

import type {
	ConvertErrorCode,
	FromConvertWorker,
	ToConvertWorker,
	ToolOp,
	ToolOptions,
} from "./worker-protocol";

export interface ConvertResult {
	blob: Blob;
	filename: string;
	mime: string;
}

export interface RunOptions {
	onProgress?: (ratio: number, stage?: string) => void;
	/** Abort the job (also tells the worker to stop and free memory). */
	signal?: AbortSignal;
}

/** A failed conversion, carrying the worker's error code so the UI can choose
 * the right response (e.g. funnel `too-large` to the desktop app). */
export class ConvertClientError extends Error {
	constructor(
		readonly code: ConvertErrorCode,
		message: string,
	) {
		super(message);
		this.name = "ConvertClientError";
	}
}

// One shared worker for all jobs; spawned on first use, reused after.
let worker: Worker | null = null;
function getWorker(): Worker {
	if (!worker) {
		worker = new Worker(new URL("./convert-worker.ts", import.meta.url), {
			type: "module",
		});
	}
	return worker;
}

let seq = 0;

/** Run one conversion to completion. Resolves with the output, rejects with a
 * `ConvertClientError`. Pass an `AbortSignal` to cancel. */
export function runConversion(
	file: File,
	op: ToolOp,
	options: ToolOptions = {},
	run: RunOptions = {},
): Promise<ConvertResult> {
	const w = getWorker();
	const id = `job-${++seq}`;

	return new Promise<ConvertResult>((resolve, reject) => {
		const onMessage = (e: MessageEvent<FromConvertWorker>) => {
			const msg = e.data;
			if (msg.id !== id) return; // not our job
			if (msg.type === "progress") {
				run.onProgress?.(msg.ratio, msg.stage);
			} else if (msg.type === "result") {
				cleanup();
				resolve({ blob: msg.blob, filename: msg.filename, mime: msg.mime });
			} else if (msg.type === "error") {
				cleanup();
				reject(new ConvertClientError(msg.code, msg.message));
			}
		};
		const onAbort = () => {
			w.postMessage({ type: "cancel", id } satisfies ToConvertWorker);
		};
		const cleanup = () => {
			w.removeEventListener("message", onMessage);
			run.signal?.removeEventListener("abort", onAbort);
		};

		if (run.signal?.aborted) {
			reject(new ConvertClientError("cancelled", "Cancelled."));
			return;
		}
		w.addEventListener("message", onMessage);
		run.signal?.addEventListener("abort", onAbort, { once: true });
		w.postMessage({ type: "run", job: { id, op, file, options } } satisfies ToConvertWorker);
	});
}

/** Tear down the shared worker (e.g. on a hard navigation away from the tools). */
export function disposeConvertWorker(): void {
	worker?.terminate();
	worker = null;
}
