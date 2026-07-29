/**
 * WebGL2 compositor backend: owns the preview shader program, its uniform
 * locations, and the full-screen quad, and draws the main compositor pass. It's
 * the reusable GL half of RenderCore — the same class the offline export
 * renderer constructs on an OffscreenCanvas, so preview and export share ONE
 * program instead of a second FFmpeg compositor. A WebGPU backend (Phase 5) will
 * expose the same beginFrame/renderMain surface.
 */

import { compile, link } from "./webgl.logic";
import { FRAG_SRC, VERT_SRC } from "./video-preview.shaders";
import { applyFrameUniforms, type UniformLocations } from "./webgl-uniforms";
import type { FrameUniforms } from "./frame-params";

/** Every uniform the shader declares; array uniforms use their `[0]` key,
 *  matching video-preview.shaders.ts. Also the completeness contract for
 *  applyFrameUniforms — a missing name here silently drops that uniform. */
const UNIFORM_NAMES = [
	"u_video", "u_background", "u_canvasSize", "u_videoOrigin", "u_videoSize",
	"u_bgType", "u_bgColor", "u_gradColors[0]", "u_gradStops[0]", "u_gradCount",
	"u_gradAngle", "u_bgBlurPx", "u_zoomCenter", "u_zoomScale", "u_motionBlurPx",
	"u_borderRadiusPx", "u_videoOpacity", "u_videoRotation", "u_cursorPos",
	"u_cursorVisible", "u_cursorRadius", "u_cursorColor", "u_highlightColor",
	"u_highlightAlpha", "u_highlightPos", "u_shadowEnabled", "u_shadowBlurPx",
	"u_shadowSpreadPx", "u_shadowOffsetPx", "u_shadowColor",
] as const;

export interface MainPassOptions {
	/** Bind the background image texture to unit 1 (image/wallpaper mode). */
	bindBackground: boolean;
	backgroundTex: WebGLTexture | null;
}

export class WebGL2Backend {
	#gl: WebGL2RenderingContext;
	#program: WebGLProgram;
	#vertexBuf: WebGLBuffer;
	#uniforms: UniformLocations;
	/** Owned frame texture for the offline export path (preview binds the frame
	 *  ring to unit 0 itself; export uploads single frames here). */
	#frameTex: WebGLTexture | null = null;

	private constructor(
		gl: WebGL2RenderingContext,
		program: WebGLProgram,
		vertexBuf: WebGLBuffer,
		uniforms: UniformLocations,
	) {
		this.#gl = gl;
		this.#program = program;
		this.#vertexBuf = vertexBuf;
		this.#uniforms = uniforms;
	}

	/** Compile the program, set up the quad + uniform-location cache, and bind the
	 *  sampler units. Throws if the shader won't compile/link. */
	static create(gl: WebGL2RenderingContext): WebGL2Backend {
		const vs = compile(gl, gl.VERTEX_SHADER, VERT_SRC);
		const fs = compile(gl, gl.FRAGMENT_SHADER, FRAG_SRC);
		const program = link(gl, vs, fs);
		gl.deleteShader(vs);
		gl.deleteShader(fs);

		const vertexBuf = gl.createBuffer()!;
		gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuf);
		gl.bufferData(
			gl.ARRAY_BUFFER,
			new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
			gl.STATIC_DRAW,
		);
		const aPos = gl.getAttribLocation(program, "a_pos");
		gl.enableVertexAttribArray(aPos);
		gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

		const uniforms: UniformLocations = {};
		for (const name of UNIFORM_NAMES) uniforms[name] = gl.getUniformLocation(program, name);

		gl.useProgram(program);
		gl.uniform1i(uniforms.u_video, 0);
		gl.uniform1i(uniforms.u_background, 1);

		return new WebGL2Backend(gl, program, vertexBuf, uniforms);
	}

	/** Set the viewport and clear to opaque black for a new frame. */
	beginFrame(width: number, height: number): void {
		const gl = this.#gl;
		gl.viewport(0, 0, width, height);
		gl.clearColor(0, 0, 0, 1);
		gl.clear(gl.COLOR_BUFFER_BIT);
	}

	/** Draw the main compositor pass. The video frame must already be bound to
	 *  texture unit 0 (the frame ring / `<video>` upload owns it). */
	renderMain(uniforms: FrameUniforms, opts: MainPassOptions): void {
		const gl = this.#gl;
		gl.useProgram(this.#program);
		if (opts.bindBackground && opts.backgroundTex) {
			gl.activeTexture(gl.TEXTURE1);
			gl.bindTexture(gl.TEXTURE_2D, opts.backgroundTex);
		}
		applyFrameUniforms(gl, this.#uniforms, uniforms);
		gl.drawArrays(gl.TRIANGLES, 0, 6);
	}

	/** Upload a decoded frame (VideoFrame/ImageBitmap/canvas) to texture unit 0 —
	 *  the offline export's equivalent of the preview's frame ring bind. */
	uploadFrame(source: TexImageSource): void {
		const gl = this.#gl;
		if (!this.#frameTex) {
			this.#frameTex = gl.createTexture();
			gl.bindTexture(gl.TEXTURE_2D, this.#frameTex);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		}
		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, this.#frameTex);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
	}

	dispose(): void {
		this.#gl.deleteBuffer(this.#vertexBuf);
		this.#gl.deleteProgram(this.#program);
		if (this.#frameTex) this.#gl.deleteTexture(this.#frameTex);
	}
}
