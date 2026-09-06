import { beforeEach, describe, expect, it } from "vitest";
import { setEditorHostHooks } from "../host-hooks";
import { CursorSmoother } from "./smoother";

class FakeWorker {
	posted: Array<{ type?: string; raw?: unknown[]; url?: string }> = [];
	onmessage: ((e: { data: unknown }) => void) | null = null;
	onerror: ((e: unknown) => void) | null = null;
	postMessage(msg: { type?: string }): void {
		this.posted.push(msg);
	}
	terminate(): void {
		// the fake worker owns no thread to stop
	}
	reply(data: unknown): void {
		this.onmessage?.({ data });
	}
}

let worker: FakeWorker;

const track = Array.from({ length: 5 }, (_, i) => ({
	timestampUs: i * 8000,
	x: i,
	y: i,
	visible: true,
	leftDown: false,
	rightDown: false,
}));

function newSmoother(): CursorSmoother {
	worker = new FakeWorker();
	setEditorHostHooks({ workers: { create: () => worker as unknown as Worker } });
	return new CursorSmoother(() => undefined);
}

describe("CursorSmoother track loading", () => {
	beforeEach(() => {
		worker = new FakeWorker();
	});

	it("ships a URL instead of the sample array when one is given", () => {
		const smoother = newSmoother();
		smoother.load(track, "asset://cursor.json");

		expect(worker.posted).toHaveLength(1);
		expect(worker.posted[0]?.type).toBe("loadUrl");
		// The whole point: the 225k-sample array never crosses the boundary.
		expect(worker.posted[0]?.raw).toBeUndefined();
	});

	it("falls back to posting the array when the worker can't read the URL", () => {
		const smoother = newSmoother();
		smoother.load(track, "asset://cursor.json");
		worker.reply({ type: "loadFailed", message: "blocked" });

		expect(worker.posted).toHaveLength(2);
		expect(worker.posted[1]?.type).toBe("load");
		expect(worker.posted[1]?.raw).toEqual(track);
	});

	it("does not re-post on a successful load", () => {
		const smoother = newSmoother();
		smoother.load(track, "asset://cursor.json");
		worker.reply({ type: "loaded" });

		expect(worker.posted).toHaveLength(1);
	});

	it("posts the array directly when no URL is available", () => {
		const smoother = newSmoother();
		smoother.load(track);

		expect(worker.posted[0]?.type).toBe("load");
		expect(worker.posted[0]?.raw).toEqual(track);
	});

	it("ignores a lifecycle message where a result was expected", () => {
		let results = 0;
		worker = new FakeWorker();
		setEditorHostHooks({ workers: { create: () => worker as unknown as Worker } });
		const smoother = new CursorSmoother(() => {
			results++;
		});
		smoother.load(track, "asset://cursor.json");
		worker.reply({ type: "loaded" });
		worker.reply({ type: "loadFailed", message: "x" });

		expect(results).toBe(0);
	});
});
