export type EngineBackend = "webgpu" | "webgl2";

/** What `PreviewEngine::create` resolves to. Mirrors the wasm-bindgen surface in
 *  `crates/recast-ffi-wasm`; the generated `.d.ts` in `wasm/` is the source of
 *  truth and this must not drift from it. */
export interface WasmPreviewEngine {
	free(): void;
	destroy(): void;
	backend(): string;
	adapterName(): string;
	isSoftware(): boolean;
	setScene(json: string): void;
	setSourceSize(width: number, height: number): void;
	screenLayerId(): number | undefined;
	cameraLayerId(): number | undefined;
	setLayerFrame(layerId: number, frame: VideoFrame): void;
	clearLayerFrame(layerId: number): void;
	setBackgroundImage(image: ImageBitmap): void;
	clearBackgroundImage(): void;
	setCursorTrack(json: string): void;
	cursorAt(outputTime: number): Float64Array;
	render(outputTime: number): number;
	outputWidth(): number;
	outputHeight(): number;
	outputDuration(): number;
}

export interface EngineModule {
	PreviewEngine: {
		create(canvas: unknown, backend?: string | null): Promise<WasmPreviewEngine>;
	};
}

/** Source UV before the zoom transform, because the sprite and the click
 *  highlight apply the zoom differently. */
export interface CursorPlacement {
	x: number;
	y: number;
	alpha: number;
	scale: number;
	pressed: boolean;
	right: boolean;
	dragging: boolean;
	highlight: { x: number; y: number; alpha: number } | null;
}

export interface NavigatorLike {
	gpu?: { requestAdapter(): Promise<unknown> };
}
