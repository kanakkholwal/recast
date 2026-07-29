import { describe, expect, it } from "vitest";
import {
	coalesceRender,
	renderWorkerCapable,
	ringNeedsRebuild,
	type ToRenderWorker,
} from "./render-worker-protocol";

const noop = () => {};

describe("renderWorkerCapable", () => {
	const full = { OffscreenCanvas: noop, VideoFrame: noop, Worker: noop };

	it("is true only when every capability is present", () => {
		expect(renderWorkerCapable(full)).toBe(true);
	});

	it("is false when any single capability is missing", () => {
		expect(renderWorkerCapable({ ...full, OffscreenCanvas: undefined })).toBe(false);
		expect(renderWorkerCapable({ ...full, VideoFrame: undefined })).toBe(false);
		expect(renderWorkerCapable({ ...full, Worker: undefined })).toBe(false);
	});
});

describe("coalesceRender", () => {
	const req = (seq: number): ToRenderWorker => ({
		type: "render",
		seq,
		uniforms: {} as never,
		bindBackgroundImage: false,
		canvasPxW: 1,
		canvasPxH: 1,
		tUs: 0,
		floorUs: 0,
		useRing: true,
		hasRenderedFrame: false,
	});

	it("sends immediately when nothing is in flight", () => {
		const r = coalesceRender(false, null, req(1));
		expect(r.send).toEqual(req(1));
	});

	it("holds and keeps only the newest while in flight (drop-late)", () => {
		let { send, pending } = coalesceRender(true, null, req(1));
		expect(send).toBeNull();
		expect((pending as { seq: number }).seq).toBe(1);
		({ send, pending } = coalesceRender(true, pending, req(2)));
		expect(send).toBeNull();
		expect((pending as { seq: number }).seq).toBe(2); // 1 was dropped, not queued
	});
});

describe("ringNeedsRebuild", () => {
	it("rebuilds on any dimension change", () => {
		expect(ringNeedsRebuild(1920, 1080, 1920, 1080)).toBe(false);
		expect(ringNeedsRebuild(1920, 1080, 3840, 2160)).toBe(true);
		expect(ringNeedsRebuild(1920, 1080, 1920, 1200)).toBe(true);
	});
});
