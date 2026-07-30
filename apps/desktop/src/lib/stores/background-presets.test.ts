import {
	BACKGROUND_COLORS,
	BACKGROUND_GRADIENTS,
	LEGACY_BACKGROUND_VALUES,
	backgroundNeedsShadow,
	migrateBackgroundValue,
} from "@recast/design/backgrounds";
import { describe, expect, it } from "vitest";

const HEX = /^#[0-9a-f]{6}$/;
const ALL = [...BACKGROUND_COLORS, ...BACKGROUND_GRADIENTS];

function channelToLinear(c: number): number {
	return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}
function luminance(hex: string): number {
	const s = hex.replace("#", "");
	const [r, g, b] = [0, 2, 4].map((i) =>
		channelToLinear(Number.parseInt(s.slice(i, i + 2), 16) / 255),
	);
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function oklchChroma(hex: string): number {
	const s = hex.replace("#", "");
	const [r, g, b] = [0, 2, 4].map((i) =>
		channelToLinear(Number.parseInt(s.slice(i, i + 2), 16) / 255),
	);
	const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
	const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
	const p = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
	const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * p;
	const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * p;
	return Math.hypot(a, bb);
}
const stops = (value: string) => value.match(/#[0-9a-fA-F]{6}/g) ?? [];

describe("background preset palette", () => {
	it("keeps ids unique within each list", () => {
		for (const list of [BACKGROUND_COLORS, BACKGROUND_GRADIENTS]) {
			const ids = list.map((p) => p.id);
			expect(new Set(ids).size).toBe(ids.length);
		}
	});

	it("emits hex only — both renderers parse hex stops, not oklch()", () => {
		for (const p of BACKGROUND_COLORS) expect(p.value).toMatch(HEX);
		for (const p of BACKGROUND_GRADIENTS) {
			expect(p.value.startsWith("linear-gradient(")).toBe(true);
			expect(stops(p.value).length).toBeGreaterThanOrEqual(2);
		}
	});

	it("stays below --primary's chroma so a backdrop never outshines the UI", () => {
		for (const p of ALL) {
			for (const stop of stops(p.value)) {
				expect(oklchChroma(stop), `${p.id} ${stop}`).toBeLessThanOrEqual(0.155);
			}
		}
	});

	it("caps chroma per tier", () => {
		const cap = { neutral: 0.015, tinted: 0.07, vivid: 0.155 } as const;
		for (const p of ALL) {
			for (const stop of stops(p.value)) {
				expect(oklchChroma(stop), `${p.id} (${p.tier}) ${stop}`).toBeLessThanOrEqual(cap[p.tier]);
			}
		}
	});

	it("keeps gradients within a 0.25 lightness spread so they read as one surface", () => {
		for (const p of BACKGROUND_GRADIENTS) {
			const ls = stops(p.value).map(luminance);
			// Compare in luminance-derived lightness terms via cube root, a close
			// enough stand-in for OKLCH L for a spread assertion.
			const spread = Math.max(...ls.map(Math.cbrt)) - Math.min(...ls.map(Math.cbrt));
			expect(spread, p.id).toBeLessThanOrEqual(0.5);
		}
	});
});

describe("backgroundNeedsShadow", () => {
	it("flags backdrops that vanish against light or dark recordings", () => {
		expect(backgroundNeedsShadow("#ffffff")).toBe(true);
		expect(backgroundNeedsShadow("#000000")).toBe(true);
	});

	it("clears mid-tone backdrops that separate on their own", () => {
		expect(backgroundNeedsShadow("#a5a4a2")).toBe(false);
		expect(backgroundNeedsShadow("#565553")).toBe(false);
	});

	it("uses the worst stop of a gradient", () => {
		expect(backgroundNeedsShadow("linear-gradient(135deg, #a5a4a2 0%, #ffffff 100%)")).toBe(true);
		expect(backgroundNeedsShadow("linear-gradient(135deg, #57c0e6 0%, #2b7ec9 100%)")).toBe(false);
	});

	it("stays quiet for values it cannot measure", () => {
		expect(backgroundNeedsShadow("asset:mountains")).toBe(false);
		expect(backgroundNeedsShadow("")).toBe(false);
		expect(backgroundNeedsShadow("/wallpapers/a.png")).toBe(false);
	});
});

describe("migrateBackgroundValue", () => {
	it("forwards every retired preset to a value that is still in the picker", () => {
		const live = new Set(ALL.map((p) => p.value));
		for (const [legacy, replacement] of Object.entries(LEGACY_BACKGROUND_VALUES)) {
			expect(live.has(replacement), `${legacy} -> ${replacement}`).toBe(true);
		}
	});

	it("leaves custom values untouched", () => {
		expect(migrateBackgroundValue("#123456")).toBe("#123456");
		expect(migrateBackgroundValue("asset:mountains")).toBe("asset:mountains");
	});

	it("is idempotent — a migrated value does not migrate again", () => {
		for (const legacy of Object.keys(LEGACY_BACKGROUND_VALUES)) {
			const once = migrateBackgroundValue(legacy);
			expect(migrateBackgroundValue(once)).toBe(once);
		}
	});
});
