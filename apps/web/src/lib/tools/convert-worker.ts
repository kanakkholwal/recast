/// <reference lib="webworker" />
/**
 * Conversion worker — the off-main-thread engine for the browser tools.
 *
 * It owns the job lifecycle: receive a job, dispatch to the op's handler, stream
 * progress, return one output Blob, and support cancellation. The actual codec
 * work for each op lives in a handler registered below; this file is the plumbing
 * those handlers plug into, so the lifecycle is right before any single tool is.
 *
 * Handlers are added one per tool as each is built. An op with no handler yet
 * fails cleanly with a clear message instead of hanging.
 */

import { handlers } from "./handlers";
import {
	ConvertError,
	type ConvertErrorCode,
	type ConvertJob,
	type FromConvertWorker,
	type JobContext,
	type ToConvertWorker,
} from "./worker-protocol";

const ctx = self as unknown as DedicatedWorkerGlobalScope;

/** In-flight jobs, for cancellation. */
const controllers = new Map<string, AbortController>();

const clamp01 = (n: number): number => Math.max(0, Math.min(1, n));

function post(msg: FromConvertWorker, transfer: Transferable[] = []): void {
	ctx.postMessage(msg, transfer);
}

function classify(
	err: unknown,
	aborted: boolean,
): {
	code: ConvertErrorCode;
	message: string;
} {
	if (err instanceof ConvertError) return { code: err.code, message: err.message };
	if (aborted) return { code: "cancelled", message: "Cancelled." };
	return {
		code: "internal",
		message: err instanceof Error ? err.message : String(err),
	};
}

async function run(job: ConvertJob): Promise<void> {
	const controller = new AbortController();
	controllers.set(job.id, controller);
	const jobCtx: JobContext = {
		signal: controller.signal,
		onProgress: (ratio, stage) =>
			post({ type: "progress", id: job.id, ratio: clamp01(ratio), stage }),
	};
	try {
		const handler = handlers[job.op];
		if (!handler) {
			throw new ConvertError("internal", `The "${job.op}" tool isn't available yet.`);
		}
		const result = await handler(job, jobCtx);
		if (controller.signal.aborted) throw new ConvertError("cancelled", "Cancelled.");
		// Blobs are structured-cloned by reference (cheap), not transferred.
		post({
			type: "result",
			id: job.id,
			blob: result.blob,
			filename: result.filename,
			mime: result.mime,
		});
	} catch (err) {
		const { code, message } = classify(err, controller.signal.aborted);
		post({ type: "error", id: job.id, code, message });
	} finally {
		controllers.delete(job.id);
	}
}

ctx.onmessage = (e: MessageEvent<ToConvertWorker>) => {
	const msg = e.data;
	if (msg.type === "run") void run(msg.job);
	else if (msg.type === "cancel") controllers.get(msg.id)?.abort();
};
