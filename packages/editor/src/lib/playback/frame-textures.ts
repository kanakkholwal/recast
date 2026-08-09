/**
 * Ring of GPU textures holding recently decoded frames.
 *
 * Why this exists: a decoded `VideoFrame` is one of the video decoder's few
 * output surfaces. Holding a handful at 4K starves the pool, and the decoder
 * responds by accepting input and emitting nothing — playback runs a second or
 * two, then freezes until a seek builds a fresh decoder. So we upload each
 * frame into a texture we own and close the frame immediately; buffer depth
 * becomes a GPU-memory decision rather than a decoder-pool one.
 */

/** Half a 60Hz frame budget: past this an upload is competing with the paint. */
const SLOW_UPLOAD_MS = 8;

/** ~5s of playback at 60fps. */
const UPLOAD_LOG_EVERY = 300;

export interface RingSlot {
	/** Presentation timestamp, microseconds. -1 when the slot is empty. */
	tsUs: number;
}

/**
 * Index of the newest slot in `[floorUs, tUs]`, or -1.
 *
 * The floor is load-bearing: frames before the current segment's start belong
 * to a removed cut, and showing one steps the picture back into deleted
 * content at every cut boundary.
 */
export function pickSlot(slots: readonly RingSlot[], tUs: number, floorUs: number): number {
	let best = -1;
	let bestTs = -1;
	for (let i = 0; i < slots.length; i++) {
		const ts = slots[i]?.tsUs ?? -1;
		if (ts < 0 || ts > tUs || ts < floorUs) continue;
		if (ts > bestTs) {
			bestTs = ts;
			best = i;
		}
	}
	return best;
}

export class FrameTextureRing {
	#gl: WebGL2RenderingContext;
	#textures: WebGLTexture[] = [];
	#slots: RingSlot[] = [];
	/** Allocated storage size per slot, parallel to `#textures`. */
	#dims: Array<{ w: number; h: number }> = [];
	#next = 0;
	#lastBound = -1;
	#uploads = 0;
	#totalUploadMs = 0;
	#maxUploadMs = 0;
	#slowUploads = 0;
	#warnedSlow = false;
	// Windowed counters, reset every report. A lifetime max never decays, so one
	// cold-start spike pins it forever and the number stops tracking reality.
	#winUploads = 0;
	#winTotalMs = 0;
	#winMaxMs = 0;
	#winSlow = 0;

	constructor(gl: WebGL2RenderingContext, capacity: number) {
		this.#gl = gl;
		for (let i = 0; i < Math.max(1, capacity); i++) {
			const tex = gl.createTexture();
			if (!tex) break;
			gl.bindTexture(gl.TEXTURE_2D, tex);
			this.#applyParams();
			this.#textures.push(tex);
			this.#slots.push({ tsUs: -1 });
			// 0×0 means "storage not allocated yet"; the first `put` sizes it.
			this.#dims.push({ w: 0, h: 0 });
		}
	}

	#applyParams(): void {
		const gl = this.#gl;
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
	}

	get capacity(): number {
		return this.#textures.length;
	}

	/**
	 * Upload `frame` into the next slot. The caller still owns the frame and
	 * must close it — this copies into GPU memory synchronously.
	 */
	put(frame: VideoFrame, tsUs: number): boolean {
		const gl = this.#gl;
		let tex = this.#textures[this.#next];
		const slot = this.#slots[this.#next];
		const dim = this.#dims[this.#next];
		if (!tex || !slot || !dim) return false;
		const w = frame.displayWidth;
		const h = frame.displayHeight;
		if (w <= 0 || h <= 0) return false;
		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, tex);
		gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
		const startedMs = performance.now();
		try {
			if (dim.w !== w || dim.h !== h) {
				// `texStorage2D` is immutable, so a size change needs a fresh texture.
				// The ring is normally rebuilt on a resolution change; this is the
				// safety net for anything that doesn't.
				if (dim.w !== 0) {
					gl.deleteTexture(tex);
					const fresh = gl.createTexture();
					if (!fresh) return false;
					tex = fresh;
					this.#textures[this.#next] = fresh;
					gl.bindTexture(gl.TEXTURE_2D, fresh);
					this.#applyParams();
				}
				// Allocate ONCE with a sized internal format. `texImage2D` per frame
				// re-specified the whole level, so the driver revalidated and could
				// reallocate 33 MB of storage on every 4K frame.
				gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, w, h);
				dim.w = w;
				dim.h = h;
			}
			gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, frame);
		} catch (err) {
			console.error("WebGL frame upload failed:", err);
			slot.tsUs = -1;
			return false;
		}
		this.#recordUpload(performance.now() - startedMs);
		slot.tsUs = tsUs;
		this.#next = (this.#next + 1) % this.#textures.length;
		return true;
	}

	/**
	 * Upload cost. This is CPU time to submit the copy, NOT GPU completion — a
	 * GPU-resident frame submits in microseconds while the blit happens later.
	 * That is exactly the signal we want: a large number here means the frame
	 * was CPU-backed and we paid a real copy on the main thread, which at 4K
	 * costs 6-17ms against a 16.6ms budget.
	 */
	#recordUpload(ms: number): void {
		this.#uploads++;
		this.#totalUploadMs += ms;
		if (ms > this.#maxUploadMs) this.#maxUploadMs = ms;
		this.#winUploads++;
		this.#winTotalMs += ms;
		if (ms > this.#winMaxMs) this.#winMaxMs = ms;
		if (ms > SLOW_UPLOAD_MS) {
			this.#slowUploads++;
			this.#winSlow++;
			if (!this.#warnedSlow) {
				this.#warnedSlow = true;
				console.warn(
					`Frame upload took ${ms.toFixed(1)}ms — the decoded frame is likely CPU-backed, ` +
						`so every frame pays a full copy on the main thread.`,
				);
			}
		}
		// Periodic in dev so a healthy pipeline is visibly healthy, rather than
		// silent (which is indistinguishable from "not running").
		if (import.meta.env.DEV && this.#winUploads >= UPLOAD_LOG_EVERY) {
			const avg = this.#winTotalMs / this.#winUploads;
			const slowPct = (this.#winSlow / this.#winUploads) * 100;
			// Report the WINDOW, not lifetime: "is it slow right now" is the only
			// actionable question, and a cumulative mean dilutes a live problem.
			console.log(
				`[ring] last ${this.#winUploads} uploads: avg ${avg.toFixed(2)}ms, ` +
					`max ${this.#winMaxMs.toFixed(2)}ms, slow ${this.#winSlow} (${slowPct.toFixed(1)}%) ` +
					`— ${this.#uploads} total, capacity ${this.capacity}`,
			);
			this.#winUploads = 0;
			this.#winTotalMs = 0;
			this.#winMaxMs = 0;
			this.#winSlow = 0;
		}
	}

	/** Session totals, for analytics. `slowCount` is the actionable one: a high
	 *  `maxMs` can be a single cold-start frame, a high `slowCount` cannot. */
	get uploadStats(): {
		count: number;
		maxMs: number;
		avgMs: number;
		slowCount: number;
		slowPct: number;
	} {
		return {
			count: this.#uploads,
			maxMs: this.#maxUploadMs,
			avgMs: this.#uploads > 0 ? this.#totalUploadMs / this.#uploads : 0,
			slowCount: this.#slowUploads,
			slowPct: this.#uploads > 0 ? (this.#slowUploads / this.#uploads) * 100 : 0,
		};
	}

	/** Bind the newest frame in `[floorUs, tUs]` to TEXTURE0. */
	bind(tUs: number, floorUs: number): boolean {
		const idx = pickSlot(this.#slots, tUs, floorUs);
		if (idx < 0) return false;
		this.#lastBound = idx;
		return this.#bindIndex(idx);
	}

	/**
	 * Re-bind the last frame we displayed. Needed because `put` binds while
	 * uploading, so unit 0 otherwise holds the newest DECODED frame — which can
	 * be ahead of the playhead or across a cut.
	 */
	bindLast(): boolean {
		return this.#lastBound >= 0 && this.#bindIndex(this.#lastBound);
	}

	#bindIndex(idx: number): boolean {
		const tex = this.#textures[idx];
		if (!tex) return false;
		this.#gl.activeTexture(this.#gl.TEXTURE0);
		this.#gl.bindTexture(this.#gl.TEXTURE_2D, tex);
		return true;
	}

	/** Drop every buffered frame — used when the source or scope changes. */
	clear(): void {
		for (const slot of this.#slots) slot.tsUs = -1;
		this.#next = 0;
		this.#lastBound = -1;
	}

	dispose(): void {
		for (const tex of this.#textures) this.#gl.deleteTexture(tex);
		this.#textures = [];
		this.#slots = [];
		this.#dims = [];
	}
}
