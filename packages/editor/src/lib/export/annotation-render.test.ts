import { describe, expect, it } from "vitest";
import {
	paintArrow,
	paintBlur,
	paintBoxAnnotation,
	type BlurEnv,
	type RenderableAnnotation,
	type RenderableBlur,
	type ShapeDeps,
} from "@recast/render";

// Recording mock of the Canvas2D surface the renderer touches: logs method calls
// so we can assert the paint sequence without a real canvas.
function mockCtx() {
	const calls: string[] = [];
	const drawArgs: unknown[][] = [];
	const rec =
		(name: string) =>
		(...args: unknown[]) => {
			calls.push(args.length ? `${name}(${args.length})` : name);
		};
	const ctx = {
		calls,
		drawArgs,
		globalAlpha: 1,
		shadowBlur: 0,
		shadowColor: "",
		fillStyle: "",
		strokeStyle: "",
		lineWidth: 0,
		lineCap: "butt",
		save: rec("save"),
		restore: rec("restore"),
		clearRect: rec("clearRect"),
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
		drawImage: (...args: unknown[]) => {
			calls.push(`drawImage(${args.length})`);
			drawArgs.push(args);
		},
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

describe("paintBlur", () => {
	const blur = (over: Partial<RenderableBlur["kind"]> = {}): RenderableBlur => ({
		opacity: 1,
		kind: { kind: "blur", strength: 0.5, variant: "none", tintColor: "", radius: 0, ...over },
	});

	function blurEnv() {
		const octx = mockCtx();
		const scratchSizes: Array<[number, number]> = [];
		const env: BlurEnv = {
			composite: {} as CanvasImageSource,
			srcW: 1920,
			srcH: 1080,
			dstW: 1920,
			dstH: 1080,
			getScratch: (w, h) => {
				scratchSizes.push([w, h]);
				return { ctx: octx as never, canvas: {} as CanvasImageSource };
			},
		};
		return { octx, scratchSizes, env };
	}

	const m0 = Math.ceil(Math.max(0.001, 0.5 * 0.12 * Math.min(1920, 1080)));

	// The scratch must include the blur margin on every side; otherwise the box
	// edges sample the canvas boundary (transparent) and corners bevel ("hexagon").
	it("blurs a margin-inclusive scratch and samples the inner box", () => {
		const outer = mockCtx();
		const { scratchSizes, env } = blurEnv();
		const box = { x: 100, y: 100, w: 200, h: 120 };
		paintBlur(outer as never, blur(), box, env);

		expect(scratchSizes[0]).toEqual([200 + 2 * m0, 120 + 2 * m0]);
		expect(outer.calls).toContain("clip");
		// 9-arg blit = sampling the inner box out of the margin, not the whole scratch.
		expect(outer.calls).toContain("drawImage(9)");
	});

	// Guards the actual geometry: the composite is sampled with the margin (so the
	// blur has real neighbours), and only the inner box (offset m) is blitted back.
	it("samples the composite with the margin and blits the inner box", () => {
		const outer = mockCtx();
		const { octx, env } = blurEnv();
		paintBlur(outer as never, blur(), { x: 100, y: 100, w: 200, h: 120 }, env);

		const sw = 200 + 2 * m0;
		const sh = 120 + 2 * m0;
		// composite → scratch: src starts a margin before the box, fills the scratch.
		expect(octx.drawArgs[0].slice(1)).toEqual([100 - m0, 100 - m0, sw, sh, 0, 0, sw, sh]);
		// scratch → overlay: skip the m-px bleed ring, land the box at (x, y, w, h).
		expect(outer.drawArgs[0].slice(1)).toEqual([m0, m0, 200, 120, 100, 100, 200, 120]);
	});

	// A box flush at the frame corner asks for source past the edge; the real
	// canvas clamps that read, and we still clip + blit the inner box (no crash).
	it("still blits the inner box for a box at the frame edge", () => {
		const outer = mockCtx();
		const { octx, env } = blurEnv();
		paintBlur(outer as never, blur(), { x: 0, y: 0, w: 200, h: 120 }, env);

		expect(octx.drawArgs[0][1]).toBe(-m0); // requests a margin past the frame edge
		expect(octx.drawArgs[0][2]).toBe(-m0);
		expect(outer.calls).toContain("clip");
		expect(outer.drawArgs[0].slice(1)).toEqual([m0, m0, 200, 120, 0, 0, 200, 120]);
	});

	it("skips a degenerate (sub-pixel) blur box", () => {
		const outer = mockCtx();
		const { env } = blurEnv();
		paintBlur(outer as never, blur(), { x: 0, y: 0, w: 1, h: 40 }, env);
		expect(outer.calls).toEqual([]);
	});
});
