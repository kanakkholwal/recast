import { describe, expect, it, vi } from "vitest";
import { RenderCore, type RenderPass } from "./render-core";
import type { WebGL2Backend } from "./webgl2-backend";
import { computeFrameParams, type FrameInput } from "./frame-params";
import type { CursorSettings, ShadowSettings } from "../stores/editor-store.svelte";

const CURSOR_OFF = {
	enabled: false,
	style: "dot",
	size: 1,
	hideWhenIdle: false,
	idleTimeout: 2,
	highlightClicks: false,
	highlightOpacity: 50,
	highlightColor: "#3b82f6",
} as unknown as CursorSettings;
const SHADOW_OFF = {
	enabled: false,
	opacity: 0,
	blur: 0,
	spread: 0,
	offsetY: 0,
	color: "#000000",
} as unknown as ShadowSettings;

function input(): FrameInput {
	return {
		meta: { width: 1920, height: 1080 },
		geom: { canvasW: 800, canvasH: 600, videoX: 40, videoY: 30, videoW: 720, videoH: 540 },
		canvasPxW: 800,
		canvasPxH: 600,
		playbackTime: 0,
		segments: [],
		segmentAnims: [],
		backgroundType: "color",
		backgroundValue: "#111111",
		backgroundBlur: 0,
		backgroundImageReady: false,
		borderRadius: 0,
		focusEnabled: false,
		zoomRegions: [],
		shadow: SHADOW_OFF,
		cursor: CURSOR_OFF,
		cursorMotionEasing: null,
		cursorSamples: [],
		idlePeriods: [],
		pressEvents: [],
	};
}

function mockBackend(seq?: string[]): WebGL2Backend {
	return {
		beginFrame: vi.fn((w: number, h: number) => seq?.push(`begin:${w}x${h}`)),
		renderMain: vi.fn(() => seq?.push("main")),
	} as unknown as WebGL2Backend;
}

describe("RenderCore.renderFrame", () => {
	it("clears, draws the main pass, then runs overlay passes in order", () => {
		const seq: string[] = [];
		const backend = mockBackend(seq);
		const passA: RenderPass = { name: "a", render: vi.fn(() => seq.push("a")) };
		const passB: RenderPass = { name: "b", render: vi.fn(() => seq.push("b")) };
		new RenderCore(backend, [passA, passB]).renderFrame(input(), { backgroundTex: null });
		expect(seq).toEqual(["begin:800x600", "main", "a", "b"]);
	});

	it("passes computeFrameParams' uniforms + bindBackground to the main pass", () => {
		const backend = mockBackend();
		const inp = input();
		new RenderCore(backend).renderFrame(inp, { backgroundTex: null });
		const expected = computeFrameParams(inp);
		expect(backend.renderMain).toHaveBeenCalledWith(expected.uniforms, {
			bindBackground: expected.bindBackgroundImage,
			backgroundTex: null,
		});
	});

	it("returns the SVG-cursor params for the DOM overlay", () => {
		const backend = mockBackend();
		const inp = input();
		const res = new RenderCore(backend).renderFrame(inp, { backgroundTex: null });
		expect(res.svgCursor).toEqual(computeFrameParams(inp).svgCursor);
	});

	it("forwards backend, params and ctx to each overlay pass", () => {
		const backend = mockBackend();
		const pass: RenderPass = { name: "p", render: vi.fn() };
		const inp = input();
		const ctx = { backgroundTex: null };
		new RenderCore(backend, [pass]).renderFrame(inp, ctx);
		expect(pass.render).toHaveBeenCalledWith(backend, computeFrameParams(inp), ctx);
	});
});
