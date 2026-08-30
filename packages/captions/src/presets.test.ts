import { describe, expect, it } from "vitest";
import { CAPTION_PRESETS } from "./presets";

function rgb(hex: string): [number, number, number] {
	const h = hex.replace("#", "");
	const n = Number.parseInt(
		h.length === 3
			? h
					.split("")
					.map((c) => c + c)
					.join("")
			: h,
		16,
	);
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function luminance(hex: string): number {
	const channel = (v: number) => {
		const s = v / 255;
		return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
	};
	const [r, g, b] = rgb(hex);
	return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function contrast(a: string, b: string): number {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (hi + 0.05) / (lo + 0.05);
}

/** The pill fill as it actually paints: the box colour composited over black. */
function pillFill(color: string, opacityPct: number): string {
	const [r, g, b] = rgb(color);
	const a = opacityPct / 100;
	const mix = (c: number) => Math.round(c * a);
	return `#${[mix(r), mix(g), mix(b)].map((c) => c.toString(16).padStart(2, "0")).join("")}`;
}

function saturation(hex: string): number {
	const [r, g, b] = rgb(hex).map((c) => c / 255);
	const max = Math.max(r, g, b);
	const min = Math.min(r, g, b);
	if (max === 0) return 0;
	return (max - min) / max;
}

describe("caption presets", () => {
	it("keeps Loom first, since it is the default style", () => {
		expect(CAPTION_PRESETS[0].id).toBe("loom");
	});

	it("has unique ids and labels", () => {
		const ids = CAPTION_PRESETS.map((p) => p.id);
		const labels = CAPTION_PRESETS.map((p) => p.label);
		expect(new Set(ids).size).toBe(ids.length);
		expect(new Set(labels).size).toBe(labels.length);
	});

	it("describes every preset, since the picker shows the description", () => {
		for (const p of CAPTION_PRESETS) {
			expect(p.description, p.id).toBeTruthy();
		}
	});

	// Subtle means short: past ~200ms it reads as a performance rather than a caption appearing.
	it("keeps entrances short", () => {
		for (const p of CAPTION_PRESETS) {
			expect(p.style.animation?.entranceMs ?? 0, p.id).toBeLessThanOrEqual(200);
		}
	});

	// Accents should read as a tint, not a highlighter; fully saturated greens and yellows dated the old set.
	it("uses restrained accent colours", () => {
		for (const p of CAPTION_PRESETS) {
			const anim = p.style.animation;
			if (!anim || anim.emphasis !== "color") continue;
			expect(saturation(anim.emphasisColor), `${p.id} accent`).toBeLessThanOrEqual(0.7);
		}
	});

	// mutedColor must clear 4.5:1 against the pill fill, not merely the spoken colour: unspoken words are still text.
	it("keeps unspoken words legible on the pill", () => {
		for (const p of CAPTION_PRESETS) {
			if (p.style.background !== "box") continue;
			if (p.style.animation?.highlight !== "progressive") continue;
			const fill = pillFill(p.style.backgroundColor, p.style.backgroundOpacity);
			expect(contrast(p.style.mutedColor, fill), `${p.id} muted on pill`).toBeGreaterThanOrEqual(
				4.5,
			);
		}
	});

	it("keeps spoken words well clear of the pill", () => {
		for (const p of CAPTION_PRESETS) {
			if (p.style.background !== "box") continue;
			const fill = pillFill(p.style.backgroundColor, p.style.backgroundOpacity);
			expect(contrast(p.style.color, fill), `${p.id} text on pill`).toBeGreaterThanOrEqual(7);
		}
	});

	// A caption that fills a third of the frame is a title card, not a caption.
	it("stays within a sane size band", () => {
		for (const p of CAPTION_PRESETS) {
			expect(p.style.fontSizePct, p.id).toBeGreaterThanOrEqual(3);
			expect(p.style.fontSizePct, p.id).toBeLessThanOrEqual(6);
		}
	});

	it("never stacks more than two lines", () => {
		for (const p of CAPTION_PRESETS) {
			expect(p.style.maxLines, p.id).toBeLessThanOrEqual(2);
		}
	});
});
