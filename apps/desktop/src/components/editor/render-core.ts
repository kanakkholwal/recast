/**
 * RenderCore: the single frame-rendering entry point over a backend + an ordered
 * pass list. One pure `computeFrameParams` evaluation drives the main compositor
 * pass, then each overlay pass (cursor sprite, camera bubble, captions,
 * annotations — folded in Phase 4 alongside the export renderer, where their
 * pixel parity is verified) draws on top. Preview and the offline export
 * renderer both drive frames through here, so there is exactly one compositor.
 */

import { computeFrameParams, type FrameInput, type FrameParams, type SvgCursorParams } from "./frame-params";
import type { WebGL2Backend } from "./webgl2-backend";

export interface RenderPassContext {
	/** Background image texture (unit 1), when the scene uses an image background. */
	backgroundTex: WebGLTexture | null;
}

/** An overlay drawn after the main pass. Kept minimal so cursor/camera/caption/
 *  annotation passes can register without touching the core. */
export interface RenderPass {
	readonly name: string;
	render(backend: WebGL2Backend, params: FrameParams, ctx: RenderPassContext): void;
}

export interface FrameResult {
	/** SVG-cursor overlay placement for the preview's HTML `<img>`; null when the
	 *  shader's dot cursor is active. (Export renders the cursor as a pass instead.) */
	svgCursor: SvgCursorParams | null;
}

export class RenderCore {
	#backend: WebGL2Backend;
	#passes: RenderPass[];

	constructor(backend: WebGL2Backend, passes: RenderPass[] = []) {
		this.#backend = backend;
		this.#passes = passes;
	}

	/** Evaluate the scene and draw the frame: main pass, then overlay passes. */
	renderFrame(input: FrameInput, ctx: RenderPassContext): FrameResult {
		const params = computeFrameParams(input);
		this.#backend.beginFrame(input.canvasPxW, input.canvasPxH);
		this.#backend.renderMain(params.uniforms, {
			bindBackground: params.bindBackgroundImage,
			backgroundTex: ctx.backgroundTex,
		});
		for (const pass of this.#passes) pass.render(this.#backend, params, ctx);
		return { svgCursor: params.svgCursor };
	}
}
