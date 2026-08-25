import { beforeEach, describe, expect, it, vi } from "vitest";
import { EngineDestroyedError, PreviewEngine } from "./preview-engine";
import type { EngineBackend, EngineModule, WasmPreviewEngine } from "./types";

function wasmStub() {
	return {
		free: vi.fn(),
		destroy: vi.fn(),
		backend: vi.fn(() => "webgl2"),
		adapterName: vi.fn(() => "SwiftShader"),
		isSoftware: vi.fn(() => true),
		setScene: vi.fn(),
		setSourceSize: vi.fn(),
		setCanvasSize: vi.fn(),
		screenLayerId: vi.fn(() => 1),
		cameraLayerId: vi.fn(() => 2),
		setLayerRingCapacity: vi.fn(),
		putLayerFrame: vi.fn(),
		bindLayerFrame: vi.fn(() => true),
		hasBoundFrame: vi.fn(() => true),
		clearLayerFrame: vi.fn(),
		setBackgroundImage: vi.fn(),
		clearBackgroundImage: vi.fn(),
		setCursorTrack: vi.fn(),
		cursorAt: vi.fn(() => new Float64Array([0.25, 0.5, 1, 0.84, 1, 0, 0, 0.3, 0.6, 0.4])),
		render: vi.fn(() => 1),
		outputWidth: vi.fn(() => 1920),
		outputHeight: vi.fn(() => 1080),
		outputDuration: vi.fn(() => 12.5),
	} satisfies WasmPreviewEngine;
}

let inner: ReturnType<typeof wasmStub>;
let create: ReturnType<typeof vi.fn>;
let loaded: EngineBackend[];

function loadModule(backend: EngineBackend): Promise<EngineModule> {
	loaded.push(backend);
	return Promise.resolve({ PreviewEngine: { create } } as unknown as EngineModule);
}

const canvas = {} as HTMLCanvasElement;

async function engine(backend?: EngineBackend | "auto", nav?: { gpu?: unknown }) {
	return PreviewEngine.create(canvas, {
		backend,
		loadModule,
		navigator: nav as never,
	});
}

beforeEach(() => {
	inner = wasmStub();
	create = vi.fn(async () => inner);
	loaded = [];
});

describe("PreviewEngine.create", () => {
	/** Loading the webgl2 artifact and then asking it for WebGPU yields a build
	 *  that has no such backend compiled in, so these two must not diverge. */
	it("loads and requests the same backend the probe settled on", async () => {
		await engine("auto", {});
		expect(loaded).toEqual(["webgl2"]);
		expect(create).toHaveBeenCalledWith(canvas, "webgl2");
	});

	it("honours an explicit backend over the probe", async () => {
		await engine("webgpu", {});
		expect(loaded).toEqual(["webgpu"]);
		expect(create).toHaveBeenCalledWith(canvas, "webgpu");
	});
});

describe("marshalling", () => {
	it("stringifies a scene object and passes a string through untouched", async () => {
		const e = await engine("webgl2");
		e.setScene({ schema: 2, layers: [] });
		expect(inner.setScene).toHaveBeenCalledWith('{"schema":2,"layers":[]}');
		e.setScene('{"raw":true}');
		expect(inner.setScene).toHaveBeenLastCalledWith('{"raw":true}');
	});

	it("reports the adapter's own answer, not the backend that was requested", async () => {
		const e = await engine("webgpu");
		expect(e.requestedBackend).toBe("webgpu");
		expect(e.backend).toBe("webgl2");
		expect(e.adapterName).toBe("SwiftShader");
		expect(e.isSoftware).toBe(true);
	});

	it("passes sizes, layers and frames straight down", async () => {
		const e = await engine("webgl2");
		e.setSourceSize(1280, 720);
		e.setCanvasSize(960, 540);
		const frame = {} as VideoFrame;
		e.setLayerRingCapacity(1, 6);
		e.putLayerFrame(1, frame, 1_500_000);
		expect(e.bindLayerFrame(1, 1_500_000, 0)).toBe(true);
		expect(e.hasBoundFrame(1)).toBe(true);
		e.clearLayerFrame(1);
		const bitmap = {} as ImageBitmap;
		e.setBackgroundImage(bitmap);
		e.clearBackgroundImage();
		expect(e.render(2.5)).toBe(1);
		expect(inner.setSourceSize).toHaveBeenCalledWith(1280, 720);
		expect(inner.setCanvasSize).toHaveBeenCalledWith(960, 540);
		expect(inner.setLayerRingCapacity).toHaveBeenCalledWith(1, 6);
		expect(inner.putLayerFrame).toHaveBeenCalledWith(1, frame, 1_500_000);
		expect(inner.bindLayerFrame).toHaveBeenCalledWith(1, 1_500_000, 0);
		expect(inner.clearLayerFrame).toHaveBeenCalledWith(1);
		expect(inner.setBackgroundImage).toHaveBeenCalledWith(bitmap);
		expect(inner.clearBackgroundImage).toHaveBeenCalled();
		expect(inner.render).toHaveBeenCalledWith(2.5);
		expect(e.outputWidth).toBe(1920);
		expect(e.outputHeight).toBe(1080);
		expect(e.outputDuration).toBe(12.5);
		expect(e.screenLayerId).toBe(1);
		expect(e.cameraLayerId).toBe(2);
	});
});

describe("cursor", () => {
	it("decodes the flat frame array into named fields", async () => {
		const e = await engine("webgl2");
		expect(e.cursorAt(1.5)).toEqual({
			x: 0.25,
			y: 0.5,
			alpha: 1,
			scale: 0.84,
			pressed: true,
			right: false,
			dragging: false,
			highlight: { x: 0.3, y: 0.6, alpha: 0.4 },
		});
		expect(inner.cursorAt).toHaveBeenCalledWith(1.5);
	});

	it("reports no cursor rather than a zeroed one when the array is empty", async () => {
		inner.cursorAt.mockReturnValue(new Float64Array([]));
		const e = await engine("webgl2");
		expect(e.cursorAt(0)).toBeNull();
	});

	/** A zero-alpha highlight is "no highlight this frame", not a black ring. */
	it("drops a highlight whose alpha has faded to nothing", async () => {
		inner.cursorAt.mockReturnValue(new Float64Array([0, 0, 1, 1, 0, 0, 0, 0.5, 0.5, 0]));
		const e = await engine("webgl2");
		expect(e.cursorAt(0)?.highlight).toBeNull();
	});

	it("stringifies a track object and passes a string through untouched", async () => {
		const e = await engine("webgl2");
		e.setCursorTrack({ samples: [] });
		expect(inner.setCursorTrack).toHaveBeenCalledWith('{"samples":[]}');
		e.setCursorTrack('{"raw":1}');
		expect(inner.setCursorTrack).toHaveBeenLastCalledWith('{"raw":1}');
	});
});

describe("lifecycle", () => {
	/** Calling into a freed wasm object throws an opaque `null pointer passed to
	 *  rust` from the glue, which is unactionable in a preview loop. */
	it("reports use after destroy rather than letting the call reach wasm", async () => {
		const e = await engine("webgl2");
		e.destroy();
		expect(e.destroyed).toBe(true);
		expect(() => e.render(0)).toThrow(EngineDestroyedError);
		expect(() => e.setScene({})).toThrow(EngineDestroyedError);
		expect(() => e.putLayerFrame(1, {} as VideoFrame, 0)).toThrow(EngineDestroyedError);
		expect(() => e.bindLayerFrame(1, 0, 0)).toThrow(EngineDestroyedError);
		expect(() => e.setBackgroundImage({} as ImageBitmap)).toThrow(EngineDestroyedError);
		expect(() => e.cursorAt(0)).toThrow(EngineDestroyedError);
		expect(() => e.backend).toThrow(EngineDestroyedError);
		expect(() => e.outputWidth).toThrow(EngineDestroyedError);
		expect(inner.render).not.toHaveBeenCalled();
		expect(inner.putLayerFrame).not.toHaveBeenCalled();
	});

	it("frees exactly once however many times destroy is called", async () => {
		const e = await engine("webgl2");
		e.destroy();
		e.destroy();
		expect(inner.free).toHaveBeenCalledTimes(1);
	});
});
