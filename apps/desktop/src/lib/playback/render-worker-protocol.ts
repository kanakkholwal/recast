/**
 * Wire contract between the main thread and the render worker (Phase 3). The
 * worker owns a WebGL2 context on its own internal OffscreenCanvas + the frame
 * ring, so decoded-frame `texImage2D` and compositing run off the main thread
 * (CON-2). It transfers a finished ImageBitmap back per frame; the main thread
 * only presents it (and can still read it back for the blur mirror). The main
 * thread keeps the clock/AV-sync and posts only precomputed uniforms.
 *
 * Decoded frames reach the worker over a MessagePort (`FramePortMessage`),
 * relayed for now and ready to wire decode→render directly (Phase 3f).
 */

import type { FrameUniforms } from "../../components/editor/frame-params";

/** Main → worker. `bitmap`/`frame` fields are transferred, not cloned. */
export type ToRenderWorker =
	| { type: "init"; ringCapacity: number }
	| { type: "framePort"; port: MessagePort }
	| {
			type: "render";
			seq: number;
			uniforms: FrameUniforms;
			bindBackgroundImage: boolean;
			/** Render-buffer size (device px); the worker sizes its canvas to match. */
			canvasPxW: number;
			canvasPxH: number;
			/** Playhead + segment-floor (µs) for ring frame selection. */
			tUs: number;
			floorUs: number;
			/** Pick from the ring; false renders background-only (no video texture). */
			useRing: boolean;
			/** Hold the last shown frame if no fresh in-window frame is ready. */
			hasRenderedFrame: boolean;
	  }
	| { type: "fallbackFrame"; frame: VideoFrame; tsUs: number }
	| { type: "background"; bitmap: ImageBitmap | null }
	| { type: "rebuildRing"; capacity: number }
	| { type: "clearRing" }
	| { type: "dispose" };

/** Worker → main. `bitmap` is transferred; present it and (optionally) read it
 *  back for the blur mirror. Absent when the worker had no frame to composite. */
export type FromRenderWorker =
	| { type: "ready" }
	| { type: "frame"; seq: number; bitmap: ImageBitmap; haveFrame: true }
	| { type: "skipped"; seq: number }
	// GL context lost (GPU reset): the worker tore its context/ring/bg down and
	// rebuilds on the next render; the client must re-send the background.
	| { type: "contextLost" }
	| { type: "error"; message: string };

/** Decode worker → render worker (over the MessageChannel port). `frame` is
 *  transferred; the worker uploads it to the ring and closes it. */
export interface FramePortMessage {
	frame: VideoFrame;
	tsUs: number;
}

/** Feature-detect the off-thread render-worker path. Takes the globals so it
 *  unit-tests without a real WebView; the caller passes `window`. `OffscreenCanvas`
 *  (worker compositing target + `transferToImageBitmap`) and `VideoFrame` (frame
 *  transfer) are the load-bearing checks; `bitmaprenderer` present is Baseline
 *  wherever both exist in WebView2. */
export function renderWorkerCapable(env: {
	OffscreenCanvas?: unknown;
	VideoFrame?: unknown;
	Worker?: unknown;
}): boolean {
	return (
		typeof env.OffscreenCanvas === "function" &&
		typeof env.VideoFrame === "function" &&
		typeof env.Worker === "function"
	);
}

/** Latest-wins mailbox for render requests: while a frame is in flight the newest
 *  request supersedes any older pending one (drop-late, never queue-and-lag).
 *  Returns the request to send now, or null to hold until the worker acks. */
export function coalesceRender(
	inFlight: boolean,
	pending: ToRenderWorker | null,
	next: ToRenderWorker,
): { send: ToRenderWorker | null; pending: ToRenderWorker | null } {
	if (inFlight) return { send: null, pending: next };
	return { send: next, pending };
}

/** True when the ring must be rebuilt for a new source resolution. */
export function ringNeedsRebuild(prevW: number, prevH: number, w: number, h: number): boolean {
	return prevW !== w || prevH !== h;
}
