import { describe, expect, it } from "vitest";
import type { OutputAspect } from "./editor/render-state";
import { computeCanvasGeometry } from "./canvas-geometry";
import cases from "../../../../fixtures/canvas-geometry.json";

interface Case {
	srcW: number;
	srcH: number;
	paddingPct: number;
	outputAspect: string | null;
	expect: Record<string, number>;
}

// Shared with `crates/recast-compositor/src/geometry.rs`: two implementations that must land on the same rects.
describe("canvas geometry parity with the Rust compositor", () => {
	it("has enough cases to catch a drift", () => {
		expect((cases as Case[]).length).toBeGreaterThanOrEqual(8);
	});

	for (const c of cases as Case[]) {
		const name = `${c.srcW}x${c.srcH} pad ${c.paddingPct} aspect ${c.outputAspect ?? "source"}`;
		it(name, () => {
			const got = computeCanvasGeometry(
				c.srcW,
				c.srcH,
				c.paddingPct,
				(c.outputAspect ?? "source") as OutputAspect,
			);
			expect({ ...got }).toEqual(c.expect);
		});
	}
});
