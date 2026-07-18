/**
 * Conversion protocol shared by apps/web's conversion handlers. Carried over
 * from `apps/web/src/lib/tools/worker-protocol.ts`; PR-B relocates the file
 * to here. (PR-A: stub definitions so the type surface is stable.)
 */

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
 * Wraps the underlying cause where applicable.
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

/**
 * Context handed to each handler: report progress (0..1) and observe
 * cancellation. The wrapper that owns the worker dispatches these.
 */
export interface JobContext {
	signal: AbortSignal;
	onProgress: (ratio: number, stage?: string) => void;
}

/** A handler returns the finished file and what to call it. */
export interface HandlerResult {
	blob: Blob;
	filename: string;
	mime: string;
}

/**
 * A conversion implementation for one op. PR-B will move the registry
 * (`trim`, `mute`, `compress`, etc.) from apps/web unchanged.
 */
export type ConvertHandler = (job: ConvertJob, ctx: JobContext) => Promise<HandlerResult>;

/** Per-op options. Loosely typed for now; each handler narrows what it needs. */
export interface ConvertJobOptions {
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
	/** The operation kind. */
	op: string;
	/** The user's file. Structured-cloned to the worker (no full copy). */
	file: File;
	options: ConvertJobOptions;
}
