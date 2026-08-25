import { type CursorPlacement, type EngineBackend, PreviewEngine } from "@recast/engine";
import type { EditorRenderState } from "../editor/render-state";

export interface EngineDriverOptions {
	canvas: HTMLCanvasElement;
	/** Overridden in tests; production resolves the real wasm artifact. */
	create?: typeof PreviewEngine.create;
	backend?: EngineBackend | "auto";
}

export interface EngineDriverInfo {
	backend: string;
	adapter: string;
	software: boolean;
}

/**
 * Owns the wasm compositor for the preview: lifecycle, scene sync and the
 * per-frame upload. Holds no render logic, and no Svelte — the component drives
 * it, so the sequencing is testable without a GPU.
 */
export class PreviewEngineDriver {
	readonly #engine: PreviewEngine;
	#sceneSignature = "";
	#trackSignature = "";
	#canvasSize = "";
	#sourceSize = "";
	#screenLayer: number | null = null;
	#ringCapacity = 0;

	private constructor(engine: PreviewEngine) {
		this.#engine = engine;
	}

	static async create(options: EngineDriverOptions): Promise<PreviewEngineDriver> {
		const create = options.create ?? PreviewEngine.create;
		const engine = await create(options.canvas, { backend: options.backend });
		return new PreviewEngineDriver(engine);
	}

	get info(): EngineDriverInfo {
		return {
			backend: this.#engine.backend,
			adapter: this.#engine.adapterName,
			software: this.#engine.isSoftware,
		};
	}

	get outputWidth(): number {
		return this.#engine.outputWidth;
	}

	get outputHeight(): number {
		return this.#engine.outputHeight;
	}

	get outputDuration(): number {
		return this.#engine.outputDuration;
	}

	/**
	 * Pushes the scene only when it actually changed. The caller runs this from a
	 * reactive effect that fires on any store write, and re-parsing an unchanged
	 * scene rebuilds the evaluator and the time map for nothing.
	 */
	syncScene(state: EditorRenderState): boolean {
		const json = JSON.stringify(state);
		if (json === this.#sceneSignature) return false;
		this.#sceneSignature = json;
		this.#engine.setScene(json);
		// The layer ids are assigned during migration, so they can move when the
		// scene changes shape.
		this.#screenLayer = this.#engine.screenLayerId ?? null;
		// The ring belongs to a layer id, so a new id means a new ring.
		this.#ringCapacity = 0;
		return true;
	}

	setSourceSize(width: number, height: number): void {
		const key = `${width}x${height}`;
		if (key === this.#sourceSize) return;
		this.#sourceSize = key;
		this.#engine.setSourceSize(width, height);
	}

	setCanvasSize(width: number, height: number): void {
		const key = `${width}x${height}`;
		if (key === this.#canvasSize) return;
		this.#canvasSize = key;
		this.#engine.setCanvasSize(width, height);
	}

	setBackgroundImage(image: ImageBitmap | null): void {
		if (image) this.#engine.setBackgroundImage(image);
		else this.#engine.clearBackgroundImage();
	}

	setCursorTrack(track: unknown | null): void {
		const json = track === null ? "" : JSON.stringify(track);
		if (json === this.#trackSignature) return;
		this.#trackSignature = json;
		this.#engine.setCursorTrack(json === "" ? '{"samples":[]}' : json);
	}

	/** Sized by the host, which knows the resolution and the memory budget. */
	setScreenRingCapacity(capacity: number): void {
		if (this.#screenLayer === null || capacity === this.#ringCapacity) return;
		this.#ringCapacity = capacity;
		this.#engine.setLayerRingCapacity(this.#screenLayer, capacity);
	}

	/** The frame is copied into a GPU texture, so the caller still owns it and
	 *  must close it. False when the scene has no screen layer yet. */
	putScreenFrame(frame: VideoFrame, timestampUs: number): boolean {
		if (this.#screenLayer === null) return false;
		this.#engine.putLayerFrame(this.#screenLayer, frame, timestampUs);
		return true;
	}

	/**
	 * Picks the decoded frame the next `render` will draw. `floorUs` is the start
	 * of the current segment: without it the picture steps back into a removed
	 * cut at every boundary. Falls back to holding the last bound frame, which is
	 * what keeps a cut from flashing the background while its GOP decodes.
	 */
	bindScreenFrame(timestampUs: number, floorUs: number): boolean {
		if (this.#screenLayer === null) return false;
		if (this.#engine.bindLayerFrame(this.#screenLayer, timestampUs, floorUs)) return true;
		return this.#engine.hasBoundFrame(this.#screenLayer);
	}

	render(outputTime: number): number {
		return this.#engine.render(outputTime);
	}

	cursorAt(outputTime: number): CursorPlacement | null {
		return this.#engine.cursorAt(outputTime);
	}

	dispose(): void {
		this.#engine.destroy();
	}
}
