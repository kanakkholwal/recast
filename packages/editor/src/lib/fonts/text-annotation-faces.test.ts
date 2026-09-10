import { describe, expect, it } from "vitest";
import type { Annotation } from "../editor/render-state";
import { textAnnotationFaces } from "./text-annotation-faces";

function text(
	id: string,
	fontFamily: string,
	fontWeight: 400 | 700 = 400,
	hidden = false,
): Annotation {
	return {
		id,
		start: 0,
		end: 1,
		rampIn: 0,
		rampOut: 0,
		easeIn: { x1: 0, y1: 0, x2: 1, y2: 1 },
		easeOut: { x1: 0, y1: 0, x2: 1, y2: 1 },
		stroke: { width: 0, color: "transparent" },
		fill: "transparent",
		hidden,
		kind: {
			kind: "text",
			x: 0,
			y: 0,
			w: 0.5,
			h: 0.1,
			content: "Ship it",
			fontFamily,
			fontSize: 0.08,
			fontWeight,
			color: "#ffffff",
			align: "center",
			lineHeight: 1.2,
		},
	};
}

function rect(id: string): Annotation {
	return {
		...text(id, "Inter"),
		kind: { kind: "rect", x: 0, y: 0, w: 0.2, h: 0.2, radius: 0 },
	};
}

describe("textAnnotationFaces", () => {
	it("asks for each family and weight once, however many use it", () => {
		const faces = textAnnotationFaces([
			text("a", "Inter", 400),
			text("b", "Inter", 400),
			text("c", "Inter", 700),
			text("d", "Anton", 400),
		]);

		expect(faces).toEqual([
			{ family: "Inter", weight: 400 },
			{ family: "Inter", weight: 700 },
			{ family: "Anton", weight: 400 },
		]);
	});

	it("ignores everything that puts no glyphs on screen", () => {
		const faces = textAnnotationFaces([
			rect("shape"),
			text("hidden", "Anton", 400, true),
			text("blank", "   "),
		]);

		expect(faces).toEqual([]);
	});

	it("trims the family, since a stray space is a different cache key but the same font", () => {
		const faces = textAnnotationFaces([text("a", "  Inter  ")]);

		expect(faces).toEqual([{ family: "Inter", weight: 400 }]);
	});
});
