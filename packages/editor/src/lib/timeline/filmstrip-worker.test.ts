import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { FromFilmstripWorker, ToFilmstripWorker } from "./filmstrip-protocol";

// MediaBunny builds a fresh `VideoDecoder` per `getCanvas` call, so overlapping
// calls mean overlapping hardware decoders. This sink counts the overlap.
const state = vi.hoisted(() => ({ live: 0, peak: 0, calls: 0 }));

vi.mock("@recast/media/mediabunny", () => ({
	ALL_FORMATS: [],
	mediaRefSource: () => ({}),
	Input: class {
		async getPrimaryVideoTrack() {
			return { getCodedWidth: async () => 1920, getCodedHeight: async () => 1080 };
		}
		async computeDuration() {
			return 60;
		}
		dispose() {}
	},
	CanvasSink: class {
		async getCanvas() {
			state.calls++;
			state.live++;
			state.peak = Math.max(state.peak, state.live);
			await new Promise((resolve) => setTimeout(resolve, 1));
			state.live--;
			return {
				canvas: { width: 160, height: 90, convertToBlob: async () => new Blob() },
			};
		}
	},
}));

let posted: FromFilmstripWorker[] = [];

type WorkerScopeStub = { onmessage?: (e: { data: ToFilmstripWorker }) => void };

function send(msg: ToFilmstripWorker): void {
	const scope = (globalThis as unknown as { self?: WorkerScopeStub }).self;
	scope?.onmessage?.({ data: msg });
}

/** The worker module is a singleton with a persistent queue, so a test must not
 *  start until the previous one's drain has fully wound down. */
async function waitFor(done: () => boolean, timeoutMs = 3000): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (done()) return;
		await new Promise((resolve) => setTimeout(resolve, 2));
	}
}

const answeredIds = () =>
	posted
		.filter((m) => m.type === "tile" || m.type === "drop" || m.type === "error")
		.map((m) => (m as { id: number }).id);

describe("filmstrip worker decode serialization", () => {
	beforeAll(async () => {
		Object.defineProperty(globalThis, "self", {
			configurable: true,
			value: {
				postMessage: (m: FromFilmstripWorker) => posted.push(m),
				onmessage: null as unknown,
			},
		});
		const mod = await import("./filmstrip-worker");
		mod.startFilmstripWorker();
	});

	beforeEach(async () => {
		send({ type: "init", src: {} as never, tileHeightPx: 90, durationSec: 60 });
		// Let any drain still running from the previous test finish before the
		// counters are zeroed, or its decodes land in this test's numbers.
		await waitFor(() => state.live === 0 && posted.some((m) => m.type === "ready"));
		await new Promise((resolve) => setTimeout(resolve, 20));
		posted = [];
		state.live = 0;
		state.peak = 0;
		state.calls = 0;
	});

	it("never runs two decodes at once, even across separate decode messages", async () => {
		// Two rAF flushes landing back to back — the pre-fix dispatcher started a
		// concurrent drain loop per message.
		send({
			type: "decode",
			requests: [
				{ id: 1, originalSec: 0 },
				{ id: 2, originalSec: 1 },
			],
		});
		send({
			type: "decode",
			requests: [
				{ id: 3, originalSec: 2 },
				{ id: 4, originalSec: 3 },
			],
		});
		await waitFor(() => answeredIds().length === 4);

		expect(state.peak).toBe(1);
		expect(posted.filter((m) => m.type === "tile")).toHaveLength(4);
	});

	it("answers every request exactly once so nothing wedges in-flight", async () => {
		send({
			type: "decode",
			requests: Array.from({ length: 12 }, (_, i) => ({ id: i, originalSec: i })),
		});
		await waitFor(() => answeredIds().length === 12);

		expect(new Set(answeredIds()).size).toBe(12);
		expect(state.peak).toBe(1);
	});

	it("drops the oldest queued requests rather than growing without bound", async () => {
		// 120 > MAX_PENDING (96): the excess must come back as `drop`, not silence.
		for (let batch = 0; batch < 6; batch++) {
			send({
				type: "decode",
				requests: Array.from({ length: 20 }, (_, i) => ({
					id: batch * 20 + i,
					originalSec: batch * 20 + i,
				})),
			});
		}
		await waitFor(() => state.live === 0 && answeredIds().length === 120);

		const drops = posted.filter((m) => m.type === "drop");
		expect(drops.length).toBeGreaterThan(0);
		expect(state.peak).toBe(1);
		// Newest-first: the last batch is nearest the viewport and must survive.
		const droppedIds = new Set(drops.map((m) => (m as { id: number }).id));
		expect(droppedIds.has(119)).toBe(false);
	});

	it("defers the storyboard until no tiles are queued", async () => {
		send({ type: "storyboard" });
		send({
			type: "decode",
			requests: Array.from({ length: 4 }, (_, i) => ({ id: i, originalSec: i })),
		});
		await waitFor(() => answeredIds().length === 4);

		const firstTile = posted.findIndex((m) => m.type === "tile");
		const storyboardStarted = posted.findIndex((m) => m.type === "storyboard");
		expect(firstTile).toBeGreaterThanOrEqual(0);
		// Tiles land before the storyboard reply (or it errored out on the missing
		// OffscreenCanvas in node — either way it did not preempt the strip).
		if (storyboardStarted >= 0) expect(storyboardStarted).toBeGreaterThan(firstTile);
		expect(state.peak).toBe(1);
	});
});
