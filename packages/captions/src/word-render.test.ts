import { describe, expect, it } from "vitest";
import { wordColor, wordScaled } from "./word-render";
import { withAlpha } from "./color";
import type { CaptionAnimation, CaptionStyle } from "./types";

const style = { color: "#ffffff", mutedColor: "#a1a1aa" } as Pick<
	CaptionStyle,
	"color" | "mutedColor"
>;

const anim = (over: Partial<CaptionAnimation>) =>
	({ highlight: "none", emphasis: "none", emphasisColor: "#4ade80", ...over }) as Pick<
		CaptionAnimation,
		"highlight" | "emphasis" | "emphasisColor"
	>;

describe("wordColor", () => {
	it("progressive: spoken words base, unspoken muted", () => {
		const a = anim({ highlight: "progressive" });
		const at = (i: number) =>
			wordColor({ index: i, activeIndex: -1, spokenCount: 2, wordCount: 4, style, anim: a });
		expect(at(0)).toBe("#ffffff");
		expect(at(1)).toBe("#ffffff");
		expect(at(2)).toBe("#a1a1aa");
	});

	it("colour emphasis wins on the active word, over progressive", () => {
		const a = anim({ highlight: "progressive", emphasis: "color" });
		// Word 2 is unspoken (would be muted) but is the active word -> accent.
		expect(
			wordColor({ index: 2, activeIndex: 2, spokenCount: 2, wordCount: 4, style, anim: a }),
		).toBe("#4ade80");
	});

	it("none / active: every non-active word uses the base colour", () => {
		const a = anim({ highlight: "active" });
		expect(
			wordColor({ index: 3, activeIndex: 1, spokenCount: 1, wordCount: 4, style, anim: a }),
		).toBe("#ffffff");
	});
});

describe("wordScaled", () => {
	it("scales the active word only for a multi-word chunk", () => {
		const a = anim({ emphasis: "scale" });
		expect(wordScaled({ index: 1, activeIndex: 1, wordCount: 3, anim: a })).toBe(true);
		expect(wordScaled({ index: 1, activeIndex: 1, wordCount: 1, anim: a })).toBe(false);
		expect(wordScaled({ index: 0, activeIndex: 1, wordCount: 3, anim: a })).toBe(false);
	});
});

describe("withAlpha", () => {
	it("mixes a hex6 with a factor", () => {
		expect(withAlpha("#000000", 0.5)).toBe("rgba(0,0,0,0.500)");
		expect(withAlpha("#ff8800", 1)).toBe("rgba(255,136,0,1.000)");
	});
	it("passes non-hex through untouched", () => {
		expect(withAlpha("transparent", 0.5)).toBe("transparent");
	});
});
