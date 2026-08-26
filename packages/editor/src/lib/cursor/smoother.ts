// Client for the cursor-smoothing worker: keeps the raw track in the worker,
// debounces + supersedes rapid slider changes, and falls back to a synchronous
// pass if the worker can't be created. Owns no rendering; callers apply the
// result (the smoothed sample array) however they like.

import { createEditorWorker } from "../host-hooks";
import { type CursorSampleLike, type SmoothingOptions, smoothCursorPath } from "./smoothing";

type ResultMsg =
	| { id: number; samples: CursorSampleLike[] }
	| { type: "loaded" }
	| { type: "loadFailed"; message: string };
type Listener = (samples: CursorSampleLike[]) => void;

// Coalesce slider drags; long enough to skip intermediate values, short enough
// to feel immediate on the first change.
const DEBOUNCE_MS = 60;

export class CursorSmoother {
	#worker: Worker | null = null;
	#raw: CursorSampleLike[] = [];
	#reqId = 0;
	#debounce: ReturnType<typeof setTimeout> | null = null;
	readonly #onResult: Listener;

	constructor(onResult: Listener) {
		this.#onResult = onResult;
		try {
			this.#worker = createEditorWorker("smoothing");
			this.#worker.onmessage = (e: MessageEvent<ResultMsg>) => {
				const msg = e.data;
				if ("type" in msg) {
					// The worker couldn't read the URL itself (asset-protocol access is
					// the host's business, not ours) — fall back to shipping the array.
					if (msg.type === "loadFailed")
						this.#worker?.postMessage({ type: "load", raw: this.#raw });
					return;
				}
				// Drop superseded results: only the latest request matters.
				if (msg.id !== this.#reqId) return;
				this.#onResult(msg.samples);
			};
			this.#worker.onerror = () => {
				this.#worker?.terminate();
				this.#worker = null;
			};
		} catch {
			this.#worker = null;
		}
	}

	/**
	 * Replace the raw track. Pass `url` and the worker re-reads the track itself:
	 * a 30-min recording is ~225k samples, and posting that array costs the main
	 * thread a full structured clone of every object at editor open. `raw` is
	 * still kept here for the no-worker fallback and for the `loadFailed` retry.
	 */
	load(raw: CursorSampleLike[], url?: string) {
		this.#raw = raw;
		if (!this.#worker) return;
		if (url) this.#worker.postMessage({ type: "loadUrl", url });
		else this.#worker.postMessage({ type: "load", raw });
	}

	/** Compute a smoothed path for `opts`, off-thread when possible. */
	request(opts: SmoothingOptions) {
		const id = ++this.#reqId;
		if (!this.#worker) {
			this.#onResult(smoothCursorPath(this.#raw, opts).samples);
			return;
		}
		if (this.#debounce !== null) clearTimeout(this.#debounce);
		this.#debounce = setTimeout(() => {
			this.#debounce = null;
			this.#worker?.postMessage({ type: "smooth", id, opts });
		}, DEBOUNCE_MS);
	}

	dispose() {
		if (this.#debounce !== null) clearTimeout(this.#debounce);
		this.#worker?.terminate();
		this.#worker = null;
	}
}
