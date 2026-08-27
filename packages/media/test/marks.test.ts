import { afterEach, describe, expect, it, vi } from "vitest";
import { markNow, measureSince } from "../src/marks";

describe("performance instrumentation (REQUIREMENTS.md §5)", () => {
	afterEach(() => vi.unstubAllGlobals());

	it("emits a prefixed measure spanning start → now", () => {
		const calls: Array<{ name: string; opts: { start: number; end: number } }> = [];
		vi.stubGlobal("performance", {
			now: () => 500,
			measure: (name: string, opts: { start: number; end: number }) => calls.push({ name, opts }),
			clearMeasures: () => {},
		});
		measureSince("seek-latency", 200);
		expect(calls).toHaveLength(1);
		expect(calls[0]?.name).toBe("recast-media:seek-latency");
		expect(calls[0]?.opts.start).toBe(200);
		expect(calls[0]?.opts.end).toBe(500);
	});

	it("clears the buffer so a long session cannot grow it without bound", () => {
		let cleared = 0;
		vi.stubGlobal("performance", {
			now: () => 1,
			measure: () => {},
			clearMeasures: () => cleared++,
		});
		for (let i = 0; i < 250; i++) measureSince("decode", 0);
		expect(cleared).toBeGreaterThan(0);
	});

	it("is inert when the Performance API is unavailable", () => {
		vi.stubGlobal("performance", undefined);
		expect(() => measureSince("decode", 0)).not.toThrow();
		expect(markNow()).toBe(0);
	});

	it("never lets a throwing measure break playback", () => {
		vi.stubGlobal("performance", {
			now: () => 1,
			measure: () => {
				throw new Error("buffer full");
			},
		});
		expect(() => measureSince("decode", 0)).not.toThrow();
	});
});
