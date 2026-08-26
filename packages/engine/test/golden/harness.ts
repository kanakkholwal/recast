/**
 * The page half of the wasm golden arm. Playwright drives `window.__golden`;
 * everything it needs (the scene, the input pixels) is passed in, so this file
 * reads no files and owns no fixture knowledge.
 *
 * Deliberately one engine reused across fixtures: an engine per fixture would
 * also be testing that a fresh adapter renders the same, which is a different
 * claim and would hide a state leak between renders.
 */

import { PreviewEngine } from "../../src/index";

interface RenderRequest {
	scene: unknown;
	outputTime: number;
	sourceWidth: number;
	sourceHeight: number;
	/** Tightly packed RGBA at the source size. */
	source: number[];
	background: number[];
}

interface RenderResult {
	width: number;
	height: number;
	/** Tightly packed RGBA, straight sRGB, the same shape as a golden PNG. */
	pixels: number[];
	layersDrawn: number;
}

declare global {
	interface Window {
		__golden: {
			backend(): Promise<string>;
			adapter(): Promise<string>;
			render(request: RenderRequest): Promise<RenderResult>;
		};
	}
}

const canvas = document.getElementById("stage") as HTMLCanvasElement;

let engine: PreviewEngine | null = null;
async function live(): Promise<PreviewEngine> {
	if (!engine) engine = await PreviewEngine.create(canvas);
	return engine;
}

/** RGBA bytes to an `ImageBitmap` the engine can take. */
async function bitmapFrom(rgba: number[], width: number, height: number): Promise<ImageBitmap> {
	const data = new ImageData(new Uint8ClampedArray(rgba), width, height);
	return createImageBitmap(data);
}

/**
 * The composited frame as straight sRGB bytes.
 *
 * Read through a 2D canvas rather than the GL context: the engine owns the
 * surface, and an inter-canvas `drawImage` is the one read that does not depend
 * on how it configured the drawing buffer.
 */
function readCanvas(width: number, height: number): number[] {
	const copy = document.createElement("canvas");
	copy.width = width;
	copy.height = height;
	const ctx = copy.getContext("2d", { willReadFrequently: true, colorSpace: "srgb" });
	if (!ctx) throw new Error("no 2d context for readback");
	ctx.drawImage(canvas, 0, 0);
	return Array.from(ctx.getImageData(0, 0, width, height, { colorSpace: "srgb" }).data);
}

window.__golden = {
	async backend() {
		return (await live()).backend;
	},
	async adapter() {
		return (await live()).adapterName;
	},
	async render(request: RenderRequest): Promise<RenderResult> {
		const e = await live();
		e.setSourceSize(request.sourceWidth, request.sourceHeight);
		e.setScene(request.scene);

		const width = e.outputWidth;
		const height = e.outputHeight;
		canvas.width = width;
		canvas.height = height;
		e.setCanvasSize(width, height);

		const background = await bitmapFrom(
			request.background,
			request.sourceWidth,
			request.sourceHeight,
		);
		e.setBackgroundImage(background);
		background.close();

		const screen = e.screenLayerId;
		if (screen === undefined) throw new Error("the scene has no screen layer");
		e.setLayerRingCapacity(screen, 2);

		const source = await bitmapFrom(request.source, request.sourceWidth, request.sourceHeight);
		const frame = new VideoFrame(source, { timestamp: 0 });
		try {
			e.putLayerFrame(screen, frame, 0);
		} finally {
			frame.close();
			source.close();
		}
		if (!e.bindLayerFrame(screen, 0, 0)) throw new Error("the source frame did not bind");

		const layersDrawn = e.render(request.outputTime);
		return { width, height, pixels: readCanvas(width, height), layersDrawn };
	},
};

// Playwright waits on this rather than on `load`, so a module-eval failure is a
// timeout with a console error rather than a silent missing global.
document.documentElement.dataset.goldenReady = "1";
