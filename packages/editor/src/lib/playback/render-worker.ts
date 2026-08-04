/**
 * Render worker (Phase 3): hosts the WebGL2 context on its OWN OffscreenCanvas
 * plus the frame-texture ring, so decoded-frame uploads (`texImage2D`) and
 * compositing run off the main thread (CON-2). It composites, then transfers a
 * finished ImageBitmap back — the main thread only presents it. Reuses the exact
 * WebGL2Backend + RenderCore the on-screen path uses, so preview stays one
 * compositor across both threads. Frames arrive over a dedicated MessagePort.
 */

import { WebGL2Backend } from "../../components/webgl2-backend";
import { RenderCore } from "../../components/render-core";
import type { FrameParams } from "../../components/frame-params";
import { FrameTextureRing } from "./frame-textures";
import type { FramePortMessage, FromRenderWorker, ToRenderWorker } from "./render-worker-protocol";

let canvas: OffscreenCanvas | null = null;
let gl: WebGL2RenderingContext | null = null;
let backend: WebGL2Backend | null = null;
let renderCore: RenderCore | null = null;
let ring: FrameTextureRing | null = null;
let ringCapacity = 6;
let bgTex: WebGLTexture | null = null;
let bgReady = false;

const post = (msg: FromRenderWorker, transfer: Transferable[] = []) =>
	(self as unknown as Worker).postMessage(msg, transfer);

function onContextLost(e: Event): void {
	// GPU reset (TDR): the context and every GL object it owned are dead. Drop
	// them so the next render rebuilds a fresh canvas/context; the ring refills
	// from incoming frames, and the client re-sends the background (only it has it).
	e.preventDefault();
	ring?.dispose();
	ring = null;
	bgTex = null;
	bgReady = false;
	backend = null;
	renderCore = null;
	gl = null;
	canvas = null;
	post({ type: "contextLost" });
}

function ensureCanvas(w: number, h: number): boolean {
	if (!gl) {
		canvas = new OffscreenCanvas(Math.max(1, w), Math.max(1, h));
		const ctx = canvas.getContext("webgl2", {
			alpha: false,
			antialias: false,
			premultipliedAlpha: false,
		});
		if (!ctx) return false;
		gl = ctx;
		canvas.addEventListener("webglcontextlost", onContextLost as EventListener);
		backend = WebGL2Backend.create(gl);
		renderCore = new RenderCore(backend);
	} else if (canvas && (canvas.width !== w || canvas.height !== h)) {
		canvas.width = Math.max(1, w);
		canvas.height = Math.max(1, h);
	}
	return true;
}

function uploadBackground(bitmap: ImageBitmap | null): void {
	if (!gl) {
		bitmap?.close();
		return;
	}
	if (!bitmap) {
		if (bgTex) gl.deleteTexture(bgTex);
		bgTex = null;
		bgReady = false;
		return;
	}
	if (!bgTex) {
		bgTex = gl.createTexture();
		gl.bindTexture(gl.TEXTURE_2D, bgTex);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
	}
	gl.bindTexture(gl.TEXTURE_2D, bgTex);
	gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
	gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, bitmap);
	bitmap.close();
	bgReady = true;
}

function onFramePort(msg: FramePortMessage): void {
	try {
		if (gl && !ring) ring = new FrameTextureRing(gl, ringCapacity);
		ring?.put(msg.frame, msg.tsUs);
	} finally {
		msg.frame.close();
	}
}

function handleRender(msg: Extract<ToRenderWorker, { type: "render" }>): void {
	if (!ensureCanvas(msg.canvasPxW, msg.canvasPxH) || !gl || !backend || !renderCore) {
		return post({ type: "skipped", seq: msg.seq });
	}
	let haveFrame = false;
	if (msg.useRing && ring) {
		haveFrame = ring.bind(Math.max(0, msg.tUs), Math.max(0, msg.floorUs));
		if (!haveFrame && msg.hasRenderedFrame) haveFrame = ring.bindLast();
	}
	// No frame bound to unit 0: skip so we never composite a stale/garbage
	// texture (main thread holds the previous presented bitmap or the spinner).
	if (!haveFrame) return post({ type: "skipped", seq: msg.seq });

	const params: FrameParams = {
		uniforms: msg.uniforms,
		svgCursor: null,
		bindBackgroundImage: msg.bindBackgroundImage && bgReady,
	};
	renderCore.applyFrameParams(params, msg.canvasPxW, msg.canvasPxH, { backgroundTex: bgTex });
	const bitmap = canvas!.transferToImageBitmap();
	post({ type: "frame", seq: msg.seq, bitmap, haveFrame: true }, [bitmap]);
}

function handle(msg: ToRenderWorker): void {
	switch (msg.type) {
		case "init":
			// GL is created lazily on the first render (needs the buffer size). Ack
			// ready so the client can start posting frames + render requests.
			ringCapacity = Math.max(1, msg.ringCapacity);
			post({ type: "ready" });
			break;
		case "framePort":
			msg.port.onmessage = (e: MessageEvent<FramePortMessage>) => onFramePort(e.data);
			break;
		case "render":
			handleRender(msg);
			break;
		case "fallbackFrame":
			onFramePort({ frame: msg.frame, tsUs: msg.tsUs });
			break;
		case "background":
			uploadBackground(msg.bitmap);
			break;
		case "rebuildRing":
			ringCapacity = Math.max(1, msg.capacity);
			ring?.dispose();
			ring = gl ? new FrameTextureRing(gl, ringCapacity) : null;
			break;
		case "clearRing":
			ring?.clear();
			break;
		case "dispose":
			ring?.dispose();
			ring = null;
			if (bgTex && gl) gl.deleteTexture(bgTex);
			bgTex = null;
			backend?.dispose();
			backend = null;
			renderCore = null;
			gl = null;
			canvas = null;
			break;
	}
}

self.onmessage = (e: MessageEvent<ToRenderWorker>) => {
	try {
		handle(e.data);
	} catch (err) {
		post({ type: "error", message: err instanceof Error ? err.message : String(err) });
	}
};
