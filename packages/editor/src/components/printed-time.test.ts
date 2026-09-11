import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const ROOT = join(import.meta.dirname, "..");

/** Formatters that print a time for a person to read. */
const FORMATTERS = ["clock", "clockCentis", "clockDecis", "fmtTime", "formatTimeByMode"];
/** The one mapping from stored source seconds to the clock the transport shows. */
const MAPPER = "store.displaySec";

/**
 * Matches a time formatter applied to a stored source-clock field, whether directly
 * (`fmtTime(a.start)`) or through the timeline's own geometry mapping
 * (`fmtTime(outSec(a.start))`), which is the render axis and can include cut gaps.
 */
function rawTimePrints(source: string): string[] {
	const field = String.raw`[\w.]+\.(?:start|end|at)\b`;
	const pattern = new RegExp(
		String.raw`\b(?:${FORMATTERS.join("|")})\(\s*(?:(\w+)\(\s*)?(${field})`,
		"g",
	);
	// `store.displaySec(` already fails the `(\w+)\(` group on its dot; only the bare prop form needs excusing.
	return (
		[...source.matchAll(pattern)]
			.filter((m) => m[1] !== "displaySec")
			// `end) - start` is a LENGTH, not a position, and a length has no clock to map to.
			.filter((m) => !/^\)\s*-/.test(source.slice((m.index ?? 0) + m[0].length)))
			.map((m) => m[0])
	);
}

function sourceFiles(dir: string): string[] {
	const out: string[] = [];
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) out.push(...sourceFiles(path));
		else if (entry.name.endsWith(".svelte") || entry.name.endsWith(".ts")) out.push(path);
	}
	return out;
}

/**
 * Annotations, zooms, caption segments, OCR spans and the playhead are stored on the
 * SOURCE clock; the transport readout and the exported file are on the output clock.
 * One cut or one speed change makes them disagree, so a panel printing `a.start`
 * showed a time nowhere near where the thing actually was.
 */
describe("printed times", () => {
	it("never formats a source-clock field without mapping it first", () => {
		const offenders = sourceFiles(ROOT)
			.filter((file) => !file.endsWith("printed-time.test.ts"))
			.flatMap((file) => {
				const relative = file.slice(ROOT.length + 1).replaceAll("\\", "/");
				return rawTimePrints(readFileSync(file, "utf8")).map(
					(hit) => `${relative}: ${hit}...) should go through ${MAPPER}`,
				);
			});

		expect(offenders).toEqual([]);
	});

	it("catches both shapes and allows the mapped one", () => {
		expect(rawTimePrints("{fmtTime(region.start)}")).toEqual(["fmtTime(region.start"]);
		expect(rawTimePrints("{formatTimeByMode(outSec(a.end), m, f)}")).toEqual([
			"formatTimeByMode(outSec(a.end",
		]);
		expect(rawTimePrints("{fmtTime(store.displaySec(region.start))}")).toEqual([]);
		expect(rawTimePrints("{fmtTime(displaySec(span.start))}")).toEqual([]);
		expect(rawTimePrints("{fmtTime(outSec(a.end) - outSec(a.start))}")).toEqual([]);
	});
});
