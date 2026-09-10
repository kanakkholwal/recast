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
	/** A few changed top-level state fields merged into the last `setScene` state; throws before any setScene. */
	patchScene(json: string): void;
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
	setTimeMap(json: string): void;
	setCursorTrack(json: string): void;
	setCaptionTrack(json: string): void;
	setCaptionFont(data: Uint8Array, index: number): boolean;
	setEditingAnnotation(id: string | undefined): void;
	setTextFont(family: string, weight: number, data: Uint8Array, index: number): boolean;
	setDrawAnnotationText(on: boolean): void;
	setCursorSprite(slot: CursorSlot, image: ImageBitmap, hotspotX: number, hotspotY: number): void;
	setAnnotationImage(path: string, image: ImageBitmap): void;
	clearAnnotationImages(): void;
	clearCursorSprites(): void;
	cursorAt(outputTime: number): Float64Array;
	render(outputTime: number): number;
	outputWidth(): number;
	outputHeight(): number;
	outputDuration(): number;
}

/** The webview's copy of a v3 project document (`crates/recast-project` compiled in). Same rules as
 *  `WasmPreviewEngine`: the `#[wasm_bindgen]` surface in `crates/recast-ffi-wasm/src/document.rs` is the truth. */
export interface WasmProjectDocument {
	free(): void;
	/** Canonical text, byte-identical to what the core writes. */
	text(): string;
	hash(): string;
	/** Applies a JSON array of ops, all or nothing; returns how many landed. Throws a string. */
	apply(opsJson: string): number;
	/** The editor render state as JSON. */
	renderState(): string;
	/** JSON array of ops that take this document to the one the state describes; `[]` when nothing moved. */
	opsForState(stateJson: string): string;
	/** The same for a few changed fields merged into the last whole state; throws before any whole state. */
	opsForPatch(patchJson: string): string;
	/** Validation findings as JSON. */
	issues(): string;
	clone(): WasmProjectDocument;
}

export interface EngineModule {
	PreviewEngine: {
		create(canvas: unknown, backend?: string | null): Promise<WasmPreviewEngine>;
	};
	ProjectDocument: {
		parse(text: string): WasmProjectDocument;
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
	gpu?: { requestAdapter(): Promise<AdapterLike | null | undefined> };
}

/** Only the part of `GPUAdapter` the probe uses. */
export interface AdapterLike {
	requestDevice(): Promise<{ destroy?(): void } | null | undefined>;
}
