/**
 * Closed set of error categories the media package throws. Each thrown
 * `MediaError` carries one of these so callers can branch on `error.code`
 * instead of string-matching messages.
 *
 * Contract (see packages/media/REQUIREMENTS.md §2 + §5):
 * - No `throw new Error("…")` in this package — always `throw new MediaError(code, …)`.
 * - `cancelled` is a control-flow signal, not a bug; callers should branch on it
 *   silently and not surface it as an error toast.
 * - `internal` means a programmer error in the package; surface to Sentry /
 *   the console, never swallow.
 */
export type MediaErrorCode =
	/** A required WebCodecs capability (decode/encode) isn't available here. */
	| "unsupported"
	/** Couldn't demux / parse the input file (unsupported container/codec). */
	| "bad-input"
	/** A decoder or encoder failed mid-stream. */
	| "decode-failed"
	/** The media worker died unexpectedly. */
	| "worker-died"
	/** Caller aborted the operation. */
	| "cancelled"
	/** Input exceeds this device's in-browser budget. */
	| "too-large"
	/** Programmer error in the package itself. */
	| "internal";

/**
 * Error class for the media package. Always prefer `throw new MediaError(code, message)`
 * over `throw new Error(message)` so consumers can branch on `code` (per
 * REQUIREMENTS.md §5). The `cause` chain is preserved.
 */
export class MediaError extends Error {
	readonly code: MediaErrorCode;

	constructor(code: MediaErrorCode, message: string, options?: { cause?: unknown }) {
		super(message, options);
		this.name = "MediaError";
		this.code = code;
	}

	/** True when the operation was cancelled by the caller (not a bug). */
	get isCancelled(): boolean {
		return this.code === "cancelled";
	}
}
