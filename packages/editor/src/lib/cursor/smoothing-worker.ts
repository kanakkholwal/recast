/// <reference lib="webworker" />
// Off-main-thread cursor smoothing. The Gaussian pass is O(N·window), which is hundreds
// of ms on a long high-σ track, so it must not run on the UI thread. The raw
// track is sent once via `load`; each slider change then ships only the tiny
// opts, and the smoothed path comes back without ever touching the main thread.

import { smoothCursorPath, type CursorSampleLike, type SmoothingOptions } from "./smoothing";

type InMsg =
	| { type: "load"; raw: CursorSampleLike[] }
	/** Fetch + parse the track here instead. A 30-min recording samples at 125Hz
	 *  → ~225k samples ≈ 27MB of JSON, and shipping that as `load` costs the main
	 *  thread a full structured clone of every object. */
	| { type: "loadUrl"; url: string }
	| { type: "smooth"; id: number; opts: SmoothingOptions };

let raw: CursorSampleLike[] = [];

async function loadFromUrl(url: string): Promise<void> {
	try {
		const res = await fetch(url);
		if (!res.ok) throw new Error(`HTTP ${res.status}`);
		const json = (await res.json()) as { samples?: CursorSampleLike[] };
		raw = json.samples ?? [];
		self.postMessage({ type: "loaded" });
	} catch (err) {
		// The host can't assume a worker may fetch its asset URL, so say so and
		// let it fall back to posting the array.
		self.postMessage({
			type: "loadFailed",
			message: err instanceof Error ? err.message : String(err),
		});
	}
}

/** Install this worker's RPC on its global scope. Called by the host app's
 *  entry module — this package never spawns a worker itself. */
export function startSmoothingWorker(): void {
	self.onmessage = (e: MessageEvent<InMsg>) => {
		const msg = e.data;
		if (msg.type === "load") {
			raw = msg.raw;
			return;
		}
		if (msg.type === "loadUrl") {
			void loadFromUrl(msg.url);
			return;
		}
		const { samples } = smoothCursorPath(raw, msg.opts);
		self.postMessage({ id: msg.id, samples });
	};
}
