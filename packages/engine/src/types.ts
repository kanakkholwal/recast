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
	setCanvasSize(width: number, height: number): void;
	screenLayerId(): number | undefined;
	cameraLayerId(): number | undefined;
	setLayerRingCapacity(layerId: number, capacity: number): void;
	putLayerFrame(layerId: number, frame: VideoFrame, timestampUs: number): void;
	bindLayerFrame(layerId: number, timestampUs: number, floorUs: number): boolean;
	hasBoundFrame(layerId: number): boolean;
	clearLayerFrame(layerId: number): void;
	setBackgroundImage(image: ImageBitmap): void;
	clearBackgroundImage(): void;
	setCursorTrack(json: string): void;
	setCursorSprite(slot: CursorSlot, image: ImageBitmap, hotspotX: number, hotspotY: number): void;
	clearCursorSprites(): void;
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

export type CursorSlot = "rest" | "press" | "rightPress" | "drag";

/** Canvas pixels. The engine draws the pointer itself; this is for placing a DOM
 *  overlay on top without re-deriving the position from the scene. */
export interface CursorPlacement {
	x: number;
	y: number;
	alpha: number;
	spritePx: number;
	dotRadiusPx: number;
	slot: CursorSlot;
	highlight: { x: number; y: number; radiusPx: number; alpha: number } | null;
}

export interface NavigatorLike {
	gpu?: { requestAdapter(): Promise<unknown> };
}
