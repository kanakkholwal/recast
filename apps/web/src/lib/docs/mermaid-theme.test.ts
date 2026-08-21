import { describe, expect, it } from "vitest";
import { MERMAID_THEME_VARIABLES, unparseableThemeColors } from "./mermaid-theme";

describe("mermaid theme", () => {
	it("passes mermaid nothing its colour maths would reject", () => {
		expect(unparseableThemeColors()).toEqual([]);
	});

	it("catches the values that broke every diagram on the site", () => {
		const broken = { primaryColor: "transparent", lineColor: "currentColor", fontSize: "14px" };

		expect(unparseableThemeColors(broken)).toEqual(["primaryColor", "lineColor"]);
	});

	it("rejects a CSS variable, which resolves in the browser but not in mermaid", () => {
		expect(unparseableThemeColors({ primaryColor: "var(--color-card)" })).toEqual(["primaryColor"]);
	});

	it("leaves non-colour settings alone", () => {
		expect(unparseableThemeColors({ fontFamily: "inherit" })).toEqual([]);
	});

	/// Guards the real config against mermaid's own colour parser, which is where
	/// the original failure surfaced rather than in any of our code.
	// Importing mermaid is ~500KB of ESM; a cold run blows the default timeout.
	it("initialises mermaid without a colour error", { timeout: 30_000 }, async () => {
		const mermaid = (await import("mermaid")).default;
		mermaid.initialize({
			startOnLoad: false,
			theme: "base",
			themeVariables: { ...MERMAID_THEME_VARIABLES },
			securityLevel: "strict",
		});

		// `parse` needs a DOM further in, so only the colour stage is asserted.
		const error = await mermaid.parse("flowchart TD\n  A[One] --> B[Two]").catch((err) => err);

		expect(String(error)).not.toMatch(/Unsupported color format/);
	});
});
