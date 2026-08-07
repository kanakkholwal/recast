/**
 * Main-thread handle to the render worker (Phase 3). The worker composites off
 * the main thread and transfers a finished ImageBitmap back; this client
 * presents it onto the on-screen canvas via a zero-copy `bitmaprenderer`
 * context. Relays decoded frames + the background image, and posts precomputed
 * uniforms with a latest-wins mailbox so a slow worker frame never queues lag.
 *
 * The on-screen canvas stays a normal main-thread canvas (not transferred), so
 * the blur-annotation mirror can still read it back.
 */

import type { FrameParams } from "../../components/frame-params";
import {
	coalesceRender,
	type FromRenderWorker,
	type ToRenderWorker,
} from "./render-worker-protocol";
import { createEditorWorker } from "../host-hooks";

export interface RenderWorkerClientOptions {
	canvas: HTMLCanvasElement;
	ringCapacity: number;
	/** First presented frame means the preview has painted (hide the spinner). */
	onPresented?: () => void;
	/** GPU reset recovered: the caller must re-send the background image. */
	onContextLost?: () => void;
	onError?: (message: string) => void;
}

export class RenderWorkerClient {
	#worker: Worker;
	#framePort: MessagePort;
	#present: ImageBitmapRenderingContext | null;
	#ready = false;
	#inFlight = false;
	#pending: ToRenderWorker | null = null;
	#seq = 0;
	#presentedOnce = false;
	#onPresented?: () => void;
	#onContextLost?: () => void;
	#onError?: (message: string) => void;

	constructor(opts: RenderWorkerClientOptions) {
		this.#onPresented = opts.onPresented;
		this.#onContextLost = opts.onContextLost;
		this.#onError = opts.onError;
		// Opaque present: our composite fills the frame, so let the browser skip
		// alpha blending on every present (MDN canvas-optimization guidance).
		this.#present = opts.canvas.getContext("bitmaprenderer", { alpha: false });
		if (!this.#present) throw new Error("bitmaprenderer context unavailable");
		this.#worker = createEditorWorker("render");
		this.#worker.onmessage = (e: MessageEvent<FromRenderWorker>) => this.#onMessage(e.data);
		this.#worker.postMessage({
			type: "init",
			ringCapacity: opts.ringCapacity,
		} satisfies ToRenderWorker);

		const channel = new MessageChannel();
		this.#framePort = channel.port1;
		this.#worker.postMessage({ type: "framePort", port: channel.port2 } satisfies ToRenderWorker, [
			channel.port2,
		]);
	}

	#onMessage(msg: FromRenderWorker): void {
		switch (msg.type) {
			case "ready":
				this.#ready = true;
				break;
			case "frame":
				this.#present?.transferFromImageBitmap(msg.bitmap);
				if (!this.#presentedOnce) {
					this.#presentedOnce = true;
					this.#onPresented?.();
				}
				this.#onAck();
				break;
			case "skipped":
				this.#onAck();
				break;
			case "contextLost":
				// The worker won't ack the in-flight render — unblock the mailbox so
				// the next frame drives the rebuild. Background must be re-sent.
				this.#inFlight = false;
				this.#pending = null;
				this.#onContextLost?.();
				break;
			case "error":
				this.#onError?.(msg.message);
				break;
		}
	}

	#onAck(): void {
		this.#inFlight = false;
		if (this.#pending) {
			const next = this.#pending;
			this.#pending = null;
			this.#post(next);
		}
	}

	#post(msg: ToRenderWorker): void {
		this.#inFlight = true;
		this.#worker.postMessage(msg);
	}

	/** Composite one frame. Latest-wins: supersedes any request still in flight. */
	renderFrame(
		params: FrameParams,
		canvasPxW: number,
		canvasPxH: number,
		tUs: number,
		floorUs: number,
		hasRenderedFrame: boolean,
		useRing = true,
	): void {
		if (!this.#ready) return;
		const req: ToRenderWorker = {
			type: "render",
			seq: this.#seq++,
			uniforms: params.uniforms,
			bindBackgroundImage: params.bindBackgroundImage,
			canvasPxW,
			canvasPxH,
			tUs,
			floorUs,
			useRing,
			hasRenderedFrame,
		};
		const { send, pending } = coalesceRender(this.#inFlight, this.#pending, req);
		this.#pending = pending;
		if (send) this.#post(send);
	}

	/** Relay a decoded frame to the worker's ring. Non-owning: clones and transfers
	 *  the clone, so the caller (whose source closes the original) keeps ownership. */
	putFrame(frame: VideoFrame, tsUs: number): void {
		const clone = frame.clone();
		this.#framePort.postMessage({ frame: clone, tsUs }, [clone]);
	}

	/** Hand a caller-owned fallback frame (e.g. `new VideoFrame(videoEl)`) to the
	 *  worker via the control channel — ordered before the next render. Ownership
	 *  moves here either way (transferred, or closed on the drop below), so the
	 *  caller must NOT close it. */
	putFallbackFrame(frame: VideoFrame, tsUs: number): void {
		// The control channel has no mailbox, unlike `renderFrame`. With a render
		// already in flight its successor is coalesced away, so posting this would
		// queue a surface the worker never composites — ~12 MB each, 60/s at 4K.
		// Drop it instead; the ring keeps the previous frame and the render's
		// `bind`/`bindLast` still finds one.
		// Before `ready` the worker has no GL context, so `ring.put` is a no-op and
		// it closes the frame on arrival — posting is pure transfer cost. This is
		// the widest window in practice: the fallback path runs during MediaBunny
		// init, which is exactly when the render worker is still starting.
		if (!this.#ready || this.#inFlight) {
			frame.close();
			return;
		}
		this.#worker.postMessage({ type: "fallbackFrame", frame, tsUs } satisfies ToRenderWorker, [
			frame,
		]);
	}

	/** Upload/clear the background image texture (bitmap is transferred). */
	setBackground(bitmap: ImageBitmap | null): void {
		this.#worker.postMessage(
			{ type: "background", bitmap } satisfies ToRenderWorker,
			bitmap ? [bitmap] : [],
		);
	}

	rebuildRing(capacity: number): void {
		this.#worker.postMessage({ type: "rebuildRing", capacity } satisfies ToRenderWorker);
	}

	clearRing(): void {
		this.#worker.postMessage({ type: "clearRing" } satisfies ToRenderWorker);
	}

	dispose(): void {
		try {
			this.#worker.postMessage({ type: "dispose" } satisfies ToRenderWorker);
		} catch {
			/* worker already gone */
		}
		this.#framePort.close();
		this.#worker.terminate();
	}
}
