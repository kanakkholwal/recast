import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { SAMPLE_CLIP } from "./sample";

// `sample.ts` copies the URL rather than importing it, to keep the landing bundle small, and this test stops that copy drifting. Read as text, since `routes/data.ts` pulls @recast/icons.
const dataSource = readFileSync(
	fileURLToPath(new URL("../../routes/data.ts", import.meta.url)),
	"utf8",
);

/** The `src` of the clip entry carrying `tone: "<tone>"`. */
function clipSrc(tone: "raw" | "polished"): string | null {
	const entries = dataSource.split(/\{\s*\n/);
	for (const entry of entries) {
		if (!entry.includes(`tone: "${tone}"`)) continue;
		const m = entry.match(/src:\s*"([^"]+)"/);
		if (m) return m[1];
	}
	return null;
}

describe("SAMPLE_CLIP", () => {
	it("matches the landing page's raw clip", () => {
		const raw = clipSrc("raw");
		expect(raw).toBeTruthy();
		expect(SAMPLE_CLIP.src).toBe(raw);
	});

	// Handing over the polished take would demo the output as the input, leaving the visitor nothing to do.
	it("is not the polished take", () => {
		const polished = clipSrc("polished");
		expect(polished).toBeTruthy();
		expect(SAMPLE_CLIP.src).not.toBe(polished);
	});

	it("is served over https, since the page is", () => {
		expect(SAMPLE_CLIP.src.startsWith("https://")).toBe(true);
	});

	it("advertises a duration and a size, so the download isn't a surprise", () => {
		expect(SAMPLE_CLIP.durationLabel).toMatch(/^\d+:\d{2}$/);
		expect(SAMPLE_CLIP.sizeLabel).toMatch(/\d/);
	});
});
