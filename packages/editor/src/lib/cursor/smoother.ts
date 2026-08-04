// Client for the cursor-smoothing worker: keeps the raw track in the worker,
// debounces + supersedes rapid slider changes, and falls back to a synchronous
// pass if the worker can't be created. Owns no rendering; callers apply the
// result (the smoothed sample array) however they like.

import {
	smoothCursorPath,
	type CursorSampleLike,
	type SmoothingOptions,
} from "./smoothing";

type ResultMsg = { id: number; samples: CursorSampleLike[] };
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
			this.#worker = new Worker(
				new URL("./smoothing-worker.ts", import.meta.url),
				{ type: "module" },
			);
			this.#worker.onmessage = (e: MessageEvent<ResultMsg>) => {
				// Drop superseded results: only the latest request matters.
				if (e.data.id !== this.#reqId) return;
				this.#onResult(e.data.samples);
			};
			this.#worker.onerror = () => {
				this.#worker?.terminate();
				this.#worker = null;
			};
		} catch {
			this.#worker = null;
		}
	}

	/** Replace the raw track; shipped to the worker once so later requests are cheap. */
	load(raw: CursorSampleLike[]) {
		this.#raw = raw;
		this.#worker?.postMessage({ type: "load", raw });
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
