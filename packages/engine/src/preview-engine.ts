import { loadEngineModule } from "./load";
import { detectBackend } from "./probe";
import type {
	CursorPlacement,
	EngineBackend,
	EngineModule,
	NavigatorLike,
	WasmPreviewEngine,
} from "./types";

export interface PreviewEngineOptions {
	backend?: EngineBackend | "auto";
	/** Injection seams for tests and for hosts that bundle the artifacts themselves. */
	loadModule?: (backend: EngineBackend) => Promise<EngineModule>;
	navigator?: NavigatorLike;
}

export class EngineDestroyedError extends Error {
	constructor() {
		super("the preview engine has been destroyed");
		this.name = "EngineDestroyedError";
	}
}

/** Lifecycle and marshalling over the wasm compositor. Holds no render logic:
 *  everything below `setScene` is evaluated in Rust. */
export class PreviewEngine {
	#inner: WasmPreviewEngine | null;
	readonly #requested: EngineBackend;

	private constructor(inner: WasmPreviewEngine, requested: EngineBackend) {
		this.#inner = inner;
		this.#requested = requested;
	}

	static async create(
		canvas: HTMLCanvasElement | OffscreenCanvas,
		options: PreviewEngineOptions = {},
	): Promise<PreviewEngine> {
		const nav = options.navigator ?? (globalThis.navigator as NavigatorLike | undefined);
		const backend = await detectBackend(nav, options.backend ?? "auto");
		const module = await (options.loadModule ?? loadEngineModule)(backend);
		return new PreviewEngine(await module.PreviewEngine.create(canvas, backend), backend);
	}

	get #live(): WasmPreviewEngine {
		if (!this.#inner) throw new EngineDestroyedError();
		return this.#inner;
	}

	get destroyed(): boolean {
		return this.#inner === null;
	}

	/** What the probe asked for. `backend` is what the adapter actually is. */
	get requestedBackend(): EngineBackend {
		return this.#requested;
	}

	get backend(): string {
		return this.#live.backend();
	}

	get adapterName(): string {
		return this.#live.adapterName();
	}

	get isSoftware(): boolean {
		return this.#live.isSoftware();
	}

	get outputWidth(): number {
		return this.#live.outputWidth();
	}

	get outputHeight(): number {
		return this.#live.outputHeight();
	}

	get outputDuration(): number {
		return this.#live.outputDuration();
	}

	get screenLayerId(): number | undefined {
		return this.#live.screenLayerId();
	}

	get cameraLayerId(): number | undefined {
		return this.#live.cameraLayerId();
	}

	/** Accepts a scene graph or a v1 render state; Rust migrates the latter. */
	setScene(scene: unknown): void {
		this.#live.setScene(typeof scene === "string" ? scene : JSON.stringify(scene));
	}

	setSourceSize(width: number, height: number): void {
		this.#live.setSourceSize(width, height);
	}

	/** The frame is copied into a GPU texture and not retained, so the caller
	 *  still owns it and must close it. */
	setLayerFrame(layerId: number, frame: VideoFrame): void {
		this.#live.setLayerFrame(layerId, frame);
	}

	clearLayerFrame(layerId: number): void {
		this.#live.clearLayerFrame(layerId);
	}

	/** Pass the bitmap at its natural size: the engine cover-fits it against the
	 *  canvas, so pre-scaling would crop it twice. Copied, not retained. */
	setBackgroundImage(image: ImageBitmap): void {
		this.#live.setBackgroundImage(image);
	}

	clearBackgroundImage(): void {
		this.#live.clearBackgroundImage();
	}

	/** The recorded pointer path, as the track file is written. */
	setCursorTrack(track: unknown): void {
		this.#live.setCursorTrack(typeof track === "string" ? track : JSON.stringify(track));
	}

	/** Null when there is no cursor to draw. Crosses the boundary as a flat
	 *  array because it is read every frame. */
	cursorAt(outputTime: number): CursorPlacement | null {
		const v = this.#live.cursorAt(outputTime);
		if (v.length === 0) return null;
		return {
			x: v[0],
			y: v[1],
			alpha: v[2],
			scale: v[3],
			pressed: v[4] === 1,
			right: v[5] === 1,
			dragging: v[6] === 1,
			highlight: v[9] > 0 ? { x: v[7], y: v[8], alpha: v[9] } : null,
		};
	}

	render(outputTime: number): number {
		return this.#live.render(outputTime);
	}

	destroy(): void {
		this.#inner?.free();
		this.#inner = null;
	}
}
