import {
	type CursorPlacement,
	type CursorSlot,
	type EngineBackend,
	PreviewEngine,
} from "@recast/engine";
import type { EditorRenderState } from "../editor/render-state";

export interface EngineDriverOptions {
	canvas: HTMLCanvasElement;
	/** Overridden in tests; production resolves the real wasm artifact. */
	create?: typeof PreviewEngine.create;
	backend?: EngineBackend | "auto";
}

export interface CursorSpriteUpload {
	slot: CursorSlot;
	image: ImageBitmap;
	/** Normalised 0..1 within the sprite. */
	hotspot: [number, number];
}

export interface EngineDriverInfo {
	backend: string;
	adapter: string;
	software: boolean;
}

/**
 * Owns the wasm compositor for the preview: lifecycle, scene sync and the
 * per-frame upload. Holds no render logic and no Svelte: the component drives
 * it, so the sequencing is testable without a GPU.
 */
export class PreviewEngineDriver {
	readonly #engine: PreviewEngine;
	#sceneSignature = "";
	#timeMapSignature = "\0";
	#trackSignature = "";
	#canvasSize = "";
	#sourceSize = "";
	#screenLayer: number | null = null;
	#cameraLayer: number | null = null;
	#cameraRing = 0;
	#ringCapacity = 0;
	#spriteKey = "";
	#captionTrackSignature = "";
	#captionFontKey = "";
	#annotationImageKey = "";

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
		try {
			this.#engine.setScene(json);
		} catch (err) {
			// Thrown from a reactive effect this would strand the engine on its last scene and silently ignore every later edit.
			console.error("preview engine refused the scene:", err);
			return false;
		}
		// Layer ids are assigned during migration, so they can move when the scene changes shape.
		this.#screenLayer = this.#engine.screenLayerId ?? null;
		this.#cameraLayer = this.#engine.cameraLayerId ?? null;
		// The ring belongs to a layer id, so a new id means a new ring.
		this.#ringCapacity = 0;
		this.#cameraRing = 0;
		return true;
	}

	/**
	 * The host's resolved output axis. The editor's cut lanes and flags can drop
	 * a cut the scene still carries, so letting the engine derive the axis for
	 * itself puts every effect at a different instant from the picture.
	 */
	setTimeMap(map: unknown | null): boolean {
		const json = map === null ? "" : JSON.stringify(map);
		if (json === this.#timeMapSignature) return false;
		this.#timeMapSignature = json;
		this.#engine.setTimeMap(json === "" ? null : json);
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

	/** Uploaded from the draw loop, so a track the engine refuses is warned
	 *  about rather than thrown: a missing pointer beats a stopped preview. */
	setCursorTrack(track: unknown | null): void {
		const json = track === null ? "" : JSON.stringify(track);
		if (json === this.#trackSignature) return;
		this.#trackSignature = json;
		try {
			this.#engine.setCursorTrack(json === "" ? '{"samples":[]}' : json);
		} catch (err) {
			console.warn("preview engine refused the cursor track:", err);
		}
	}

	/** Same upload shape as the cursor track: bulky, its own channel, and a
	 *  refusal warns rather than throws so captions failing cannot stop the
	 *  preview. */
	setCaptionTrack(words: unknown | null): void {
		const json = words === null ? "" : JSON.stringify(words);
		if (json === this.#captionTrackSignature) return;
		this.#captionTrackSignature = json;
		try {
			this.#engine.setCaptionTrack(json);
		} catch (err) {
			console.warn("preview engine refused the caption track:", err);
		}
	}

	/**
	 * The font file captions are drawn with. Required: wasm has no filesystem to
	 * resolve a CSS family against, so the host resolves it natively and ships
	 * the bytes. `key` is the family plus weight, so a re-upload only happens
	 * when the style actually picks a different face.
	 */
	setCaptionFont(key: string, data: Uint8Array): boolean {
		if (key === this.#captionFontKey) return false;
		if (!this.#engine.setCaptionFont(data, 0)) {
			console.warn("preview engine could not read the caption font", key);
			return false;
		}
		this.#captionFontKey = key;
		return true;
	}

	/**
	 * Replaces the pointer sprites. `key` identifies the set (the style id), so
	 * rasterising and re-uploading only happens when the style actually changes.
	 * An empty list clears them, which puts the engine back on the dot.
	 */
	setCursorSprites(key: string, sprites: CursorSpriteUpload[]): boolean {
		if (key === this.#spriteKey) return false;
		this.#spriteKey = key;
		this.#engine.clearCursorSprites();
		for (const sprite of sprites) {
			this.#engine.setCursorSprite(sprite.slot, sprite.image, sprite.hotspot);
		}
		return true;
	}

	/**
	 * Replaces the decoded assets for image annotations. `key` identifies the
	 * set, so re-decoding only happens when the paths actually change. Keyed by
	 * path rather than by annotation id: two annotations on one file share the
	 * upload, which is what the host's own image cache already assumes.
	 */
	setAnnotationImages(key: string, images: Map<string, ImageBitmap>): boolean {
		if (key === this.#annotationImageKey) return false;
		this.#annotationImageKey = key;
		this.#engine.clearAnnotationImages();
		for (const [path, image] of images) {
			this.#engine.setAnnotationImage(path, image);
		}
		return true;
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

	/**
	 * The camera feed for this instant. It comes off a `<video>` the host keeps
	 * seeked to the playhead rather than a decode stream, so there is nothing to
	 * buffer: one slot, uploaded and bound in the same tick. False when the scene
	 * has no camera layer, which is every recording made without one.
	 */
	putCameraFrame(frame: VideoFrame, timestampUs: number): boolean {
		if (this.#cameraLayer === null) return false;
		if (this.#cameraRing === 0) {
			this.#cameraRing = 1;
			this.#engine.setLayerRingCapacity(this.#cameraLayer, 1);
		}
		this.#engine.putLayerFrame(this.#cameraLayer, frame, timestampUs);
		// Floor 0: the camera is its own track and cuts never remove part of it.
		return this.#engine.bindLayerFrame(this.#cameraLayer, timestampUs, 0);
	}

	/** True once a camera frame has been bound, so the host can tell "no camera
	 *  in this project" from "the element has not decoded yet". */
	hasCameraFrame(): boolean {
		return this.#cameraLayer !== null && this.#engine.hasBoundFrame(this.#cameraLayer);
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
