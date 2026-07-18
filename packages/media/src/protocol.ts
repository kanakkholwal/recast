/**
 * Shared protocol types for the conversion tools. The web app's
 * `apps/web/src/lib/tools/worker-protocol.ts` becomes a thin re-export of this
 * module after PR-B.
 *
 * What lives here:
 * - The op kinds (`ToolOp`) and their options (`ToolOptions`) — the handler
 *   registry in `./handlers` keys off these.
 * - The job, result, error, and progress shapes the worker + UI pass around.
 * - The wire types (`ToConvertWorker` / `FromConvertWorker`) — generic "worker
 *   RPC" patterns; even if apps/web's worker is the only consumer today, they
 *   belong with the protocol.
 *
 * Cancellation is signalled via `AbortSignal` (`JobContext.signal`) — that's
 * the standard primitive — and mirrored in the wire message so the worker can
 * map `cancel` -> `controller.abort()` deterministically.
 */

/** The conversion operations the worker can run. Grouped by capability tier
 *  (see `capabilities.ts` in apps/web): container ops need no codec, decode ops
 *  need a decoder, encode ops need a `VideoEncoder` (Chromium-first). */
export type ToolOp =
	// Tier A — container only (no WebCodecs): rewrap/copy streams.
	| 'trim' // cut [start, end], keyframe-aligned stream copy
	| 'mute' // drop the audio track
	| 'extract-audio' // pull the audio track out as-is (no re-encode)
	// Tier B — decode only: read frames/samples, encode with a small JS/WASM codec.
	| 'video-to-gif'
	| 'audio-to-mp3'
	| 'extract-frames' // frames → PNG/JPG (zip or single)
	// Tier C — encode (VideoEncoder): touch pixels and re-encode.
	| 'transcode' // change codec/container (mp4 <-> webm, mov -> mp4)
	| 'compress' // re-encode at a lower bitrate
	| 'resize'; // scale dimensions

/** Per-op options. Loosely typed for now; each handler narrows what it needs. */
export interface ToolOptions {
	/** trim: seconds. */
	startSec?: number;
	endSec?: number;
	/** video-to-gif / resize: target dimensions (height auto if omitted). */
	width?: number;
	height?: number;
	/** video-to-gif: output frame rate, e.g. 10–15. */
	fps?: number;
	/** transcode/compress: target container + codecs. */
	container?: 'mp4' | 'webm';
	videoCodec?: string;
	audioCodec?: string;
	/** compress: target average bitrate (bits/sec). */
	videoBitrate?: number;
	/** extract-audio / audio-to-mp3: output format. */
	audioFormat?: 'mp3' | 'wav' | 'm4a';
	/** extract-frames: still format + how many evenly-spaced frames. */
	imageFormat?: 'png' | 'jpeg';
	frameCount?: number;
}

/** A single conversion request. */
export interface ConvertJob {
	/** Caller-assigned id, echoed on every message for this job. */
	id: string;
	op: ToolOp;
	/** The user's file. Structured-cloned to the worker (no full copy). */
	file: File;
	options: ToolOptions;
}

/**
 * Context handed to each handler: report progress (0..1) and observe cancel.
 * The wrapper that owns the worker dispatches these.
 */
export interface JobContext {
	signal: AbortSignal;
	onProgress: (ratio: number, stage?: string) => void;
}

/**
 * Codes drive the user-facing error message and whether to funnel to the
 * desktop app (`too-large`) or suggest another browser (`unsupported`).
 */
export type ConvertErrorCode =
	/** A required WebCodecs capability isn't available here. */
	| 'unsupported'
	/** Input exceeds this device's in-browser budget. */
	| 'too-large'
	/** Couldn't demux/decode the file (unsupported container/codec). */
	| 'bad-input'
	/** Caller cancelled. */
	| 'cancelled'
	/** Programmer error in the package. */
	| 'internal';

/**
 * Thrown inside a handler to fail a job with a specific, user-facing code.
 * Wraps the underlying cause where applicable. Cancellation is signalled by
 * `MediaError` with `code: 'cancelled'` (via `JobContext.signal.aborted`)
 * — the worker maps that into this error before posting to the main thread.
 */
export class ConvertError extends Error {
	readonly code: ConvertErrorCode;

	constructor(code: ConvertErrorCode, message: string, options?: { cause?: unknown }) {
		super(message, options);
		this.name = 'ConvertError';
		this.code = code;
	}

	/** True when the operation was cancelled by the caller (not a bug). */
	get isCancelled(): boolean {
		return this.code === 'cancelled';
	}
}

/** What a handler returns: the finished file and what to call it. */
export interface HandlerResult {
	blob: Blob;
	filename: string;
	mime: string;
}

/**
 * A conversion implementation for one op. The handler receives a job + a
 * context (abort signal + progress reporter) and resolves with the finished
 * file or rejects with a `ConvertError`.
 */
export type ConvertHandler = (job: ConvertJob, ctx: JobContext) => Promise<HandlerResult>;

/** Main thread -> worker. Generic "worker RPC" shape; lives here so any
 *  `@recast/media`-driven worker (web app today, future in-browser editor)
 *  can speak the same wire format. */
export type ToConvertWorker = { type: 'run'; job: ConvertJob } | { type: 'cancel'; id: string };

/** Worker -> main thread. `progress` carries 0..1 + an optional stage label;
 *  terminal events are `result` or `error`. */
export type FromConvertWorker =
	| { type: 'progress'; id: string; ratio: number; stage?: string }
	| {
			type: 'result';
			id: string;
			blob: Blob;
			filename: string;
			mime: string;
	  }
	| { type: 'error'; id: string; code: ConvertErrorCode; message: string };
