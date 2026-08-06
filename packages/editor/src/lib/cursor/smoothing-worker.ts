/// <reference lib="webworker" />
// Off-main-thread cursor smoothing. The Gaussian pass is O(N·window), which is hundreds
// of ms on a long high-σ track, so it must not run on the UI thread. The raw
// track is sent once via `load`; each slider change then ships only the tiny
// opts, and the smoothed path comes back without ever touching the main thread.

import { smoothCursorPath, type CursorSampleLike, type SmoothingOptions } from "./smoothing";

type InMsg =
	| { type: "load"; raw: CursorSampleLike[] }
	| { type: "smooth"; id: number; opts: SmoothingOptions };

let raw: CursorSampleLike[] = [];

/** Install this worker's RPC on its global scope. Called by the host app's
 *  entry module — this package never spawns a worker itself. */
export function startSmoothingWorker(): void {
	self.onmessage = (e: MessageEvent<InMsg>) => {
		const msg = e.data;
		if (msg.type === "load") {
			raw = msg.raw;
			return;
		}
		const { samples } = smoothCursorPath(raw, msg.opts);
		self.postMessage({ id: msg.id, samples });
	};
}
