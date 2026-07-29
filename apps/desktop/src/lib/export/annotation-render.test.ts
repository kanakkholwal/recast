import { describe, expect, it } from "vitest";
import {
	paintArrow,
	paintBoxAnnotation,
	type RenderableAnnotation,
	type ShapeDeps,
} from "@recast/render";

// Recording mock of the Canvas2D surface the renderer touches: logs method calls
// so we can assert the paint sequence without a real canvas.
function mockCtx() {
	const calls: string[] = [];
	const rec =
		(name: string) =>
		(...args: unknown[]) => {
			calls.push(args.length ? `${name}(${args.length})` : name);
		};
	const ctx = {
		calls,
		globalAlpha: 1,
		shadowBlur: 0,
		shadowColor: "",
		fillStyle: "",
		strokeStyle: "",
		lineWidth: 0,
		lineCap: "butt",
		save: rec("save"),
		restore: rec("restore"),
		beginPath: rec("beginPath"),
		closePath: rec("closePath"),
		rect: rec("rect"),
		ellipse: rec("ellipse"),
		moveTo: rec("moveTo"),
		lineTo: rec("lineTo"),
		quadraticCurveTo: rec("quadraticCurveTo"),
		fill: rec("fill"),
		stroke: rec("stroke"),
		fillRect: rec("fillRect"),
		strokeRect: rec("strokeRect"),
		drawImage: rec("drawImage"),
		clip: rec("clip"),
		setLineDash: rec("setLineDash"),
	};
	return ctx;
}

const noImages: ShapeDeps = { getImage: () => null };
const box = { x: 10, y: 20, w: 100, h: 50 };

function anno(over: Partial<RenderableAnnotation>): RenderableAnnotation {
	return {
		stroke: { width: 0.01, color: "#ff0000" },
		fill: "#00ff00",
		kind: { kind: "rect" },
		...over,
	};
}

describe("paintBoxAnnotation", () => {
	it("fills and strokes a rect within a save/restore", () => {
		const ctx = mockCtx();
		paintBoxAnnotation(ctx as never, anno({ kind: { kind: "rect" } }), box, 1000, 0.8, noImages);
		expect(ctx.calls[0]).toBe("save");
		expect(ctx.calls.at(-1)).toBe("restore");
		expect(ctx.calls).toContain("rect(4)");
		expect(ctx.calls).toContain("fill");
		expect(ctx.calls).toContain("stroke");
		expect(ctx.globalAlpha).toBe(0.8);
	});

	it("uses a rounded path when the rect has a radius", () => {
		const ctx = mockCtx();
		paintBoxAnnotation(
			ctx as never,
			anno({ kind: { kind: "rect", radius: 0.2 } }),
			box,
			1000,
			1,
			noImages,
		);
		expect(ctx.calls).toContain("quadraticCurveTo(4)"); // roundRectPath
		expect(ctx.calls).not.toContain("rect(4)");
	});

	it("draws an ellipse for the ellipse kind", () => {
		const ctx = mockCtx();
		paintBoxAnnotation(ctx as never, anno({ kind: { kind: "ellipse" } }), box, 1000, 1, noImages);
		expect(ctx.calls.some((c) => c.startsWith("ellipse"))).toBe(true);
	});

	it("skips a degenerate (zero-size) box", () => {
		const ctx = mockCtx();
		paintBoxAnnotation(ctx as never, anno({}), { x: 0, y: 0, w: 0, h: 10 }, 1000, 1, noImages);
		expect(ctx.calls).toEqual([]);
	});

	it("does not fill/stroke a transparent shape", () => {
		const ctx = mockCtx();
		paintBoxAnnotation(
			ctx as never,
			anno({ fill: "transparent", stroke: { width: 0, color: "transparent" } }),
			box,
			1000,
			1,
			noImages,
		);
		expect(ctx.calls).not.toContain("fill");
		expect(ctx.calls).not.toContain("stroke");
	});

	it("draws a ready image and its border", () => {
		const ctx = mockCtx();
		const deps: ShapeDeps = { getImage: () => ({ img: {} as CanvasImageSource, ready: true }) };
		paintBoxAnnotation(
			ctx as never,
			anno({ kind: { kind: "image", path: "/x.png", opacity: 1, radius: 0 } }),
			box,
			1000,
			1,
			deps,
		);
		expect(ctx.calls).toContain("drawImage(5)");
		expect(ctx.calls).toContain("stroke"); // image border
	});

	it("draws a placeholder when the image is not ready", () => {
		const ctx = mockCtx();
		const deps: ShapeDeps = { getImage: () => ({ img: {} as CanvasImageSource, ready: false }) };
		paintBoxAnnotation(
			ctx as never,
			anno({ kind: { kind: "image", path: "/x.png" }, stroke: { width: 0, color: "transparent" } }),
			box,
			1000,
			1,
			deps,
		);
		expect(ctx.calls).toContain("fillRect(4)");
		expect(ctx.calls).not.toContain("drawImage(4)");
	});
});

describe("paintArrow", () => {
	it("strokes the shaft and fills the head", () => {
		const ctx = mockCtx();
		paintArrow(
			ctx as never,
			anno({ kind: { kind: "arrow", headSize: 0.15 } }),
			{ x: 0, y: 0 },
			{ x: 200, y: 0 },
			1000,
			1,
		);
		expect(ctx.calls).toContain("stroke"); // shaft
		expect(ctx.calls).toContain("fill"); // head triangle
		expect(ctx.calls[0]).toBe("save");
		expect(ctx.calls.at(-1)).toBe("restore");
	});

	it("skips a degenerate (sub-pixel) arrow", () => {
		const ctx = mockCtx();
		paintArrow(
			ctx as never,
			anno({ kind: { kind: "arrow" } }),
			{ x: 5, y: 5 },
			{ x: 5, y: 5 },
			1000,
			1,
		);
		expect(ctx.calls).toEqual([]);
	});
});
