import { beforeEach, describe, expect, it, vi } from "vitest";
import type { EditorRenderState } from "../editor/render-state";
import { PreviewEngineDriver } from "./engine-driver";

function engineStub() {
	return {
		backend: "webgpu",
		adapterName: "Test Adapter",
		isSoftware: false,
		outputWidth: 1920,
		outputHeight: 1080,
		outputDuration: 9,
		screenLayerId: 3 as number | undefined,
		cameraLayerId: 4,
		destroyed: false,
		setScene: vi.fn(),
		setTimeMap: vi.fn(),
		setSourceSize: vi.fn(),
		setCanvasSize: vi.fn(),
		setBackgroundImage: vi.fn(),
		clearBackgroundImage: vi.fn(),
		setCursorTrack: vi.fn(),
		setCursorSprite: vi.fn(),
		setAnnotationImage: vi.fn(),
		clearAnnotationImages: vi.fn(),
		clearCursorSprites: vi.fn(),
		setLayerRingCapacity: vi.fn(),
		putLayerFrame: vi.fn(),
		bindLayerFrame: vi.fn(() => true),
		hasBoundFrame: vi.fn(() => false),
		clearLayerFrame: vi.fn(),
		cursorAt: vi.fn(() => null),
		render: vi.fn(() => 1),
		destroy: vi.fn(),
	};
}

let engine: ReturnType<typeof engineStub>;
const canvas = {} as HTMLCanvasElement;

function driver() {
	return PreviewEngineDriver.create({
		canvas,
		create: (async () => engine) as never,
	});
}

const state = (padding: number) => ({ padding }) as unknown as EditorRenderState;

beforeEach(() => {
	engine = engineStub();
});

describe("scene sync", () => {
	/** The caller runs this from an effect that fires on ANY store write, so an
	 *  unchanged scene must not rebuild the evaluator and the time map. */
	it("pushes a scene once and skips an identical one", async () => {
		const d = await driver();
		expect(d.syncScene(state(4))).toBe(true);
		expect(d.syncScene(state(4))).toBe(false);
		expect(d.syncScene(state(8))).toBe(true);
		expect(engine.setScene).toHaveBeenCalledTimes(2);
	});

	/** Thrown from the effect that calls this, a refused scene would strand the
	 *  engine on the last one it took and silence every later edit. */
	it("reports a refused scene rather than throwing into the effect", async () => {
		const d = await driver();
		const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
		engine.setScene.mockImplementation(() => {
			throw new Error("missing field padding");
		});
		expect(d.syncScene(state(4))).toBe(false);
		expect(error).toHaveBeenCalled();
		error.mockRestore();
	});

	/** Layer ids are assigned during migration, so they can move whenever the
	 *  scene changes shape. Caching the first one would upload frames to a layer
	 *  that no longer exists. */
	it("re-reads the screen layer id on every accepted scene", async () => {
		const d = await driver();
		d.syncScene(state(4));
		engine.screenLayerId = 7;
		d.syncScene(state(8));
		const frame = {} as VideoFrame;
		d.putScreenFrame(frame, 1000);
		expect(engine.putLayerFrame).toHaveBeenCalledWith(7, frame, 1000);
	});

	it("refuses a frame while the scene has no screen layer", async () => {
		engine.screenLayerId = undefined;
		const d = await driver();
		d.syncScene(state(4));
		expect(d.putScreenFrame({} as VideoFrame, 0)).toBe(false);
		expect(engine.putLayerFrame).not.toHaveBeenCalled();
	});
});

describe("frame ring", () => {
	/** A new scene can move the screen layer id, and the ring belongs to an id,
	 *  so the capacity has to be re-sent rather than assumed still applied. */
	it("re-sends the ring capacity when the screen layer id moves", async () => {
		const d = await driver();
		d.syncScene(state(4));
		d.setScreenRingCapacity(6);
		d.setScreenRingCapacity(6);
		expect(engine.setLayerRingCapacity).toHaveBeenCalledTimes(1);
		engine.screenLayerId = 9;
		d.syncScene(state(8));
		d.setScreenRingCapacity(6);
		expect(engine.setLayerRingCapacity).toHaveBeenLastCalledWith(9, 6);
	});

	/** A camera toggle (or any edit) changes the scene but keeps the screen layer
	 *  id. Rebuilding the ring then would drop the bound frame and flash the
	 *  screen black, so the populated ring must survive an unchanged id. */
	it("keeps the ring across a scene change that does not move the screen layer id", async () => {
		const d = await driver();
		d.syncScene(state(4));
		d.setScreenRingCapacity(6);
		expect(engine.setLayerRingCapacity).toHaveBeenCalledTimes(1);
		d.syncScene(state(8));
		d.setScreenRingCapacity(6);
		expect(engine.setLayerRingCapacity).toHaveBeenCalledTimes(1);
	});

	/** Right after a cut the post-cut GOP has not decoded yet. Holding the last
	 *  frame is what stops the picture flashing the background at every cut. */
	it("holds the last bound frame when nothing new qualifies", async () => {
		const d = await driver();
		d.syncScene(state(4));
		engine.bindLayerFrame.mockReturnValue(false);
		engine.hasBoundFrame.mockReturnValue(true);
		expect(d.bindScreenFrame(1000, 0)).toBe(true);
		engine.hasBoundFrame.mockReturnValue(false);
		expect(d.bindScreenFrame(1000, 0)).toBe(false);
	});

	it("passes the segment floor through so a cut cannot show a removed frame", async () => {
		const d = await driver();
		d.syncScene(state(4));
		d.bindScreenFrame(5_000_000, 2_000_000);
		expect(engine.bindLayerFrame).toHaveBeenCalledWith(3, 5_000_000, 2_000_000);
	});
});

describe("cursor sprites", () => {
	const sprite = (slot: "rest" | "press") => ({
		slot,
		image: {} as ImageBitmap,
		hotspot: [0.25, 0.75] as [number, number],
	});

	it("uploads a set once and skips the same key", async () => {
		const d = await driver();
		expect(d.setCursorSprites("arrow", [sprite("rest"), sprite("press")])).toBe(true);
		expect(d.setCursorSprites("arrow", [sprite("rest")])).toBe(false);
		expect(engine.setCursorSprite).toHaveBeenCalledTimes(2);
		expect(engine.setCursorSprite).toHaveBeenCalledWith("rest", expect.anything(), [0.25, 0.75]);
	});

	/** Slots are not overwritten in place, so a style with fewer states than the
	 *  last one would keep showing the old sprite for the missing slots. */
	it("clears the previous set before uploading a new one", async () => {
		const d = await driver();
		d.setCursorSprites("arrow", [sprite("rest"), sprite("press")]);
		d.setCursorSprites("hand", [sprite("rest")]);
		expect(engine.clearCursorSprites).toHaveBeenCalledTimes(2);
		expect(engine.setCursorSprite).toHaveBeenCalledTimes(3);
	});

	it("clears back to the dot on an empty set", async () => {
		const d = await driver();
		d.setCursorSprites("arrow", [sprite("rest")]);
		d.setCursorSprites("dot", []);
		expect(engine.clearCursorSprites).toHaveBeenCalledTimes(2);
		expect(engine.setCursorSprite).toHaveBeenCalledTimes(1);
	});
});

describe("camera frames", () => {
	/** A `<video>` seeked to the playhead, not a decode stream: nothing to
	 *  buffer, so one slot, uploaded and bound in the same tick. */
	it("sizes the ring once and binds every frame", async () => {
		const d = await driver();
		d.syncScene(state(4));
		const frame = {} as VideoFrame;
		expect(d.putCameraFrame(frame, 1000)).toBe(true);
		expect(d.putCameraFrame(frame, 2000)).toBe(true);
		expect(engine.setLayerRingCapacity).toHaveBeenCalledTimes(1);
		expect(engine.setLayerRingCapacity).toHaveBeenCalledWith(4, 1);
		expect(engine.putLayerFrame).toHaveBeenLastCalledWith(4, frame, 2000);
		expect(engine.bindLayerFrame).toHaveBeenLastCalledWith(4, 2000, 0);
	});

	/** A new scene can move the camera layer id, and the ring belongs to an id. */
	it("re-sizes the ring when the camera layer id moves", async () => {
		const d = await driver();
		d.syncScene(state(4));
		d.putCameraFrame({} as VideoFrame, 1000);
		engine.cameraLayerId = 9;
		d.syncScene(state(8));
		d.putCameraFrame({} as VideoFrame, 2000);
		expect(engine.setLayerRingCapacity).toHaveBeenLastCalledWith(9, 1);
	});

	/** Toggling the overlay changes the scene but keeps the camera layer id, so
	 *  the one-slot ring must not be rebuilt — that would drop the bound frame. */
	it("keeps the camera ring across a scene change that does not move the camera layer id", async () => {
		const d = await driver();
		d.syncScene(state(4));
		d.putCameraFrame({} as VideoFrame, 1000);
		expect(engine.setLayerRingCapacity).toHaveBeenCalledTimes(1);
		d.syncScene(state(8));
		d.putCameraFrame({} as VideoFrame, 2000);
		expect(engine.setLayerRingCapacity).toHaveBeenCalledTimes(1);
	});

	it("refuses a frame while the scene has no camera layer", async () => {
		engine.cameraLayerId = undefined as unknown as number;
		const d = await driver();
		d.syncScene(state(4));
		expect(d.putCameraFrame({} as VideoFrame, 0)).toBe(false);
		expect(d.hasCameraFrame()).toBe(false);
		expect(engine.putLayerFrame).not.toHaveBeenCalled();
	});
});

describe("time map", () => {
	/** The editor drops cuts its own lane flags disable, so the engine must be
	 *  told what output time means rather than deriving it from the scene. */
	it("sends a map once and skips an identical one", async () => {
		const d = await driver();
		const map = {
			spans: [{ origStart: 0, origEnd: 4, speed: 1, outStart: 0, outEnd: 4 }],
			outputDuration: 4,
		};
		expect(d.setTimeMap(map)).toBe(true);
		expect(d.setTimeMap({ ...map })).toBe(false);
		expect(engine.setTimeMap).toHaveBeenCalledTimes(1);
	});

	/** A never-sent map and a cleared one are different states: the first frame
	 *  must not be evaluated on an axis the host never agreed to. */
	it("clears back to the scene's own axis", async () => {
		const d = await driver();
		expect(d.setTimeMap(null)).toBe(true);
		expect(engine.setTimeMap).toHaveBeenCalledWith(null);
		expect(d.setTimeMap(null)).toBe(false);
	});
});

describe("annotation images", () => {
	const bitmap = () => ({}) as ImageBitmap;

	it("uploads a set once and skips the same key", async () => {
		const d = await driver();
		const set = new Map([["a.png", bitmap()]]);
		expect(d.setAnnotationImages("a.png", set)).toBe(true);
		expect(d.setAnnotationImages("a.png", set)).toBe(false);
		expect(engine.setAnnotationImage).toHaveBeenCalledTimes(1);
		expect(engine.setAnnotationImage).toHaveBeenCalledWith("a.png", expect.anything());
	});

	/** Slots are keyed by path, so an annotation whose image was swapped would
	 *  keep drawing the old file unless the whole set is cleared first. */
	it("clears the previous set before uploading a new one", async () => {
		const d = await driver();
		d.setAnnotationImages("a.png", new Map([["a.png", bitmap()]]));
		d.setAnnotationImages("b.png", new Map([["b.png", bitmap()]]));
		expect(engine.clearAnnotationImages).toHaveBeenCalledTimes(2);
		expect(engine.setAnnotationImage).toHaveBeenLastCalledWith("b.png", expect.anything());
	});

	it("clears back to nothing when the last image annotation goes", async () => {
		const d = await driver();
		d.setAnnotationImages("a.png", new Map([["a.png", bitmap()]]));
		d.setAnnotationImages("", new Map());
		expect(engine.clearAnnotationImages).toHaveBeenCalledTimes(2);
		expect(engine.setAnnotationImage).toHaveBeenCalledTimes(1);
	});
});

describe("size sync", () => {
	it("sends each size once", async () => {
		const d = await driver();
		d.setSourceSize(1280, 720);
		d.setSourceSize(1280, 720);
		d.setCanvasSize(640, 360);
		d.setCanvasSize(640, 360);
		d.setCanvasSize(1280, 720);
		expect(engine.setSourceSize).toHaveBeenCalledTimes(1);
		expect(engine.setCanvasSize).toHaveBeenCalledTimes(2);
	});
});

describe("background and cursor", () => {
	it("clears rather than uploading when the image is gone", async () => {
		const d = await driver();
		const bitmap = {} as ImageBitmap;
		d.setBackgroundImage(bitmap);
		d.setBackgroundImage(null);
		expect(engine.setBackgroundImage).toHaveBeenCalledWith(bitmap);
		expect(engine.clearBackgroundImage).toHaveBeenCalled();
	});

	it("uploads a cursor track once and skips an identical one", async () => {
		const d = await driver();
		d.setCursorTrack({ samples: [1] });
		d.setCursorTrack({ samples: [1] });
		expect(engine.setCursorTrack).toHaveBeenCalledTimes(1);
	});

	/** Called from the draw loop: a track the engine refuses must not take the
	 *  whole preview down with it. */
	it("warns rather than throwing when the engine refuses a track", async () => {
		const d = await driver();
		const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
		engine.setCursorTrack.mockImplementation(() => {
			throw new Error("missing field x");
		});
		expect(() => d.setCursorTrack({ samples: [1] })).not.toThrow();
		expect(warn).toHaveBeenCalled();
		warn.mockRestore();
	});

	/** Clearing must reach the engine as an empty track, not as the string
	 *  "null", which would fail to parse and leave the old cursor drawing. */
	it("clears the cursor track with an empty one", async () => {
		const d = await driver();
		d.setCursorTrack({ samples: [1] });
		d.setCursorTrack(null);
		expect(engine.setCursorTrack).toHaveBeenLastCalledWith('{"samples":[]}');
	});
});

describe("reporting", () => {
	it("passes the adapter through for the backend probe", async () => {
		const d = await driver();
		expect(d.info).toEqual({ backend: "webgpu", adapter: "Test Adapter", software: false });
		expect(d.outputWidth).toBe(1920);
		expect(d.outputHeight).toBe(1080);
		expect(d.outputDuration).toBe(9);
	});
});
