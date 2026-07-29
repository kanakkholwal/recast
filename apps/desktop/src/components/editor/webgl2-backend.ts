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
import { OverlayQuad, type QuadDrawOptions, type QuadRect } from "./overlay-quad";
import type { FrameUniforms } from "./frame-params";

/** Every uniform the shader declares; array uniforms use their `[0]` key,
 *  matching video-preview.shaders.ts. Also the completeness contract for
 *  applyFrameUniforms — a missing name here silently drops that uniform. */
const UNIFORM_NAMES = [
	"u_video",
	"u_background",
	"u_canvasSize",
	"u_videoOrigin",
	"u_videoSize",
	"u_bgType",
	"u_bgColor",
	"u_gradColors[0]",
	"u_gradStops[0]",
	"u_gradCount",
	"u_gradAngle",
	"u_bgBlurPx",
	"u_zoomCenter",
	"u_zoomScale",
	"u_motionBlurPx",
	"u_borderRadiusPx",
	"u_videoOpacity",
	"u_videoRotation",
	"u_cursorPos",
	"u_cursorVisible",
	"u_cursorRadius",
	"u_cursorColor",
	"u_highlightColor",
	"u_highlightAlpha",
	"u_highlightPos",
	"u_shadowEnabled",
	"u_shadowBlurPx",
	"u_shadowSpreadPx",
	"u_shadowOffsetPx",
	"u_shadowColor",
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
	#aPos: number;
	#uniforms: UniformLocations;
	/** Lazily created on first overlay draw (export only; preview has none). */
	#overlay: OverlayQuad | null = null;
	#canvasW = 0;
	#canvasH = 0;
	/** Owned frame texture for the offline export path (preview binds the frame
	 *  ring to unit 0 itself; export uploads single frames here). */
	#frameTex: WebGLTexture | null = null;
	/** Owned background-image texture for the offline export path (image/wallpaper
	 *  backgrounds). The preview owns its own `bgTex`; export uploads once here. */
	#bgTex: WebGLTexture | null = null;
	/** Owned camera-frame texture (export camera bubble); re-uploaded each frame. */
	#camTex: WebGLTexture | null = null;
	/** Owned annotation-layer texture (export); re-uploaded each frame. */
	#annoTex: WebGLTexture | null = null;

	private constructor(
		gl: WebGL2RenderingContext,
		program: WebGLProgram,
		vertexBuf: WebGLBuffer,
		aPos: number,
		uniforms: UniformLocations,
	) {
		this.#gl = gl;
		this.#program = program;
		this.#vertexBuf = vertexBuf;
		this.#aPos = aPos;
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

		return new WebGL2Backend(gl, program, vertexBuf, aPos, uniforms);
	}

	/** Set the viewport and clear to opaque black for a new frame. */
	beginFrame(width: number, height: number): void {
		const gl = this.#gl;
		this.#canvasW = width;
		this.#canvasH = height;
		gl.viewport(0, 0, width, height);
		gl.clearColor(0, 0, 0, 1);
		gl.clear(gl.COLOR_BUFFER_BIT);
	}

	/** Draw the main compositor pass. The video frame must already be bound to
	 *  texture unit 0 (the frame ring / `<video>` upload owns it). */
	renderMain(uniforms: FrameUniforms, opts: MainPassOptions): void {
		const gl = this.#gl;
		gl.useProgram(this.#program);
		// Rebind the main quad each frame: overlay passes bind their own buffer,
		// and there's no VAO to isolate the attrib state.
		gl.bindBuffer(gl.ARRAY_BUFFER, this.#vertexBuf);
		gl.enableVertexAttribArray(this.#aPos);
		gl.vertexAttribPointer(this.#aPos, 2, gl.FLOAT, false, 0, 0);
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

	/** Upload the background image (image/wallpaper mode) to an owned texture and
	 *  return it, to hand back as `RenderPassContext.backgroundTex` each frame. */
	uploadBackground(source: TexImageSource): WebGLTexture {
		const gl = this.#gl;
		if (!this.#bgTex) {
			this.#bgTex = gl.createTexture();
			gl.bindTexture(gl.TEXTURE_2D, this.#bgTex);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		}
		gl.bindTexture(gl.TEXTURE_2D, this.#bgTex);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
		return this.#bgTex;
	}

	/** Upload an arbitrary image (sprite/caption/annotation) to a new straight-alpha
	 *  texture. The caller owns disposal via {@link deleteTexture}. */
	createTextureFrom(source: TexImageSource): WebGLTexture {
		const gl = this.#gl;
		const tex = gl.createTexture()!;
		gl.bindTexture(gl.TEXTURE_2D, tex);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
		return tex;
	}

	deleteTexture(tex: WebGLTexture): void {
		this.#gl.deleteTexture(tex);
	}

	/** Upload a camera frame to the owned camera texture (export bubble), reused
	 *  across frames, and return it to draw with {@link drawSprite}. */
	uploadCamera(source: TexImageSource): WebGLTexture {
		const gl = this.#gl;
		if (!this.#camTex) {
			this.#camTex = gl.createTexture();
			gl.bindTexture(gl.TEXTURE_2D, this.#camTex);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		}
		gl.bindTexture(gl.TEXTURE_2D, this.#camTex);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
		return this.#camTex;
	}

	/** Upload the comp-native annotation layer to the owned texture (export),
	 *  reused across frames, and return it for the annotation pass to composite. */
	uploadAnnotation(source: TexImageSource): WebGLTexture {
		const gl = this.#gl;
		if (!this.#annoTex) {
			this.#annoTex = gl.createTexture();
			gl.bindTexture(gl.TEXTURE_2D, this.#annoTex);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
			gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		}
		gl.bindTexture(gl.TEXTURE_2D, this.#annoTex);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
		return this.#annoTex;
	}

	/** Blit a texture into a pixel-space rect over the current frame (overlay
	 *  passes). Lazily compiles the overlay program on first use. */
	drawSprite(tex: WebGLTexture, rect: QuadRect, opts?: QuadDrawOptions): void {
		if (!this.#overlay) this.#overlay = OverlayQuad.create(this.#gl);
		this.#overlay.draw(tex, rect, this.#canvasW, this.#canvasH, opts);
	}

	dispose(): void {
		this.#gl.deleteBuffer(this.#vertexBuf);
		this.#gl.deleteProgram(this.#program);
		this.#overlay?.dispose();
		if (this.#frameTex) this.#gl.deleteTexture(this.#frameTex);
		if (this.#bgTex) this.#gl.deleteTexture(this.#bgTex);
		if (this.#camTex) this.#gl.deleteTexture(this.#camTex);
		if (this.#annoTex) this.#gl.deleteTexture(this.#annoTex);
	}
}
