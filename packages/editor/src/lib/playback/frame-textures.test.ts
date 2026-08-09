import { describe, expect, it } from "vitest";
import { FrameTextureRing, pickSlot, type RingSlot } from "./frame-textures";

const slots = (...ts: number[]): RingSlot[] => ts.map((tsUs) => ({ tsUs }));

describe("pickSlot", () => {
	it("returns the newest frame at or before the playhead", () => {
		// A ring wraps, so slot order is not timestamp order.
		const ring = slots(3_000, 4_000, 1_000, 2_000);
		expect(pickSlot(ring, 2_500, 0)).toBe(3); // 2_000
		expect(pickSlot(ring, 4_000, 0)).toBe(1); // 4_000
	});

	it("never returns a frame ahead of the playhead", () => {
		expect(pickSlot(slots(5_000, 6_000), 1_000, 0)).toBe(-1);
	});

	it("never returns a frame before the segment floor", () => {
		// The floor is the start of the current kept segment; anything earlier
		// is inside a removed cut and would step the picture backwards.
		const ring = slots(1_000, 9_000);
		expect(pickSlot(ring, 10_000, 5_000)).toBe(1);
		expect(pickSlot(ring, 4_000, 5_000)).toBe(-1);
	});

	it("ignores empty slots", () => {
		expect(pickSlot(slots(-1, -1, -1), 10_000, 0)).toBe(-1);
		expect(pickSlot(slots(-1, 2_000, -1), 10_000, 0)).toBe(1);
	});

	it("accepts a frame exactly on the playhead and on the floor", () => {
		expect(pickSlot(slots(2_000), 2_000, 0)).toBe(0);
		expect(pickSlot(slots(2_000), 5_000, 2_000)).toBe(0);
	});

	it("handles an empty ring", () => {
		expect(pickSlot([], 1_000, 0)).toBe(-1);
	});
});

/** Records the calls the upload path is judged on. */
function fakeGl() {
	const calls = { storage: [] as Array<[number, number]>, sub: 0, created: 0, deleted: 0 };
	const gl = {
		TEXTURE_2D: 1,
		TEXTURE0: 2,
		RGBA: 3,
		RGBA8: 4,
		UNSIGNED_BYTE: 5,
		TEXTURE_WRAP_S: 6,
		TEXTURE_WRAP_T: 7,
		TEXTURE_MIN_FILTER: 8,
		TEXTURE_MAG_FILTER: 9,
		CLAMP_TO_EDGE: 10,
		LINEAR: 11,
		UNPACK_PREMULTIPLY_ALPHA_WEBGL: 12,
		createTexture: () => {
			calls.created++;
			return { id: calls.created };
		},
		deleteTexture: () => {
			calls.deleted++;
		},
		bindTexture() {},
		activeTexture() {},
		texParameteri() {},
		pixelStorei() {},
		texStorage2D: (_t: number, _l: number, _f: number, w: number, h: number) => {
			calls.storage.push([w, h]);
		},
		texSubImage2D: () => {
			calls.sub++;
		},
	};
	return { gl: gl as unknown as WebGL2RenderingContext, calls };
}

const frame = (w: number, h: number) =>
	({ displayWidth: w, displayHeight: h }) as unknown as VideoFrame;

describe("FrameTextureRing upload", () => {
	it("allocates storage once per slot, then only sub-uploads", () => {
		const { gl, calls } = fakeGl();
		const ring = new FrameTextureRing(gl, 2);
		for (let i = 0; i < 10; i++) ring.put(frame(3840, 2160), i * 1000);

		// 2 slots -> 2 allocations, not one per frame.
		expect(calls.storage).toEqual([
			[3840, 2160],
			[3840, 2160],
		]);
		expect(calls.sub).toBe(10);
	});

	it("reallocates a slot when the frame size changes", () => {
		const { gl, calls } = fakeGl();
		const ring = new FrameTextureRing(gl, 1);
		ring.put(frame(1920, 1080), 0);
		ring.put(frame(3840, 2160), 1000);

		expect(calls.storage).toEqual([
			[1920, 1080],
			[3840, 2160],
		]);
		// Immutable storage can't be resized, so the texture is replaced.
		expect(calls.deleted).toBe(1);
	});

	it("rejects a degenerate frame instead of allocating zero-sized storage", () => {
		const { gl, calls } = fakeGl();
		const ring = new FrameTextureRing(gl, 1);
		expect(ring.put(frame(0, 0), 0)).toBe(false);
		expect(calls.storage).toEqual([]);
		expect(calls.sub).toBe(0);
	});
});
