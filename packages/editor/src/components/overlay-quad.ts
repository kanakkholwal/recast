/**
 * Textured-quad blitter for the export overlay passes (cursor sprite, camera
 * bubble, captions, annotation images). Draws one straight-alpha texture into a
 * pixel-space rect with per-draw opacity and an optional rounded-rect / circle
 * mask, over the main compositor pass. Its own program + VAO-free attrib, rebound
 * each draw, so it never clobbers the main pass's quad (which also rebinds).
 */

import { compile, link } from "./webgl.logic";

const VERT = `#version 300 es
in vec2 a_quad;
uniform vec4 u_rect;    // x, y, w, h in pixels (origin top-left)
uniform vec2 u_canvas;  // canvas size in pixels
out vec2 v_uv;
void main() {
	v_uv = a_quad;
	vec2 px = u_rect.xy + a_quad * u_rect.zw;
	vec2 ndc = (px / u_canvas) * 2.0 - 1.0;
	gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
}`;

const FRAG = `#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_tex;
uniform float u_alpha;
uniform vec2 u_sizePx;    // rect w,h in px, for the corner SDF
uniform float u_radiusPx; // rounded-corner radius; >= min(w,h)/2 → circle
uniform vec4 u_uvRect;    // sub-rect of the texture to sample: u0,v0,du,dv
out vec4 frag;

float roundedMask(vec2 uv, vec2 sizePx, float rPx) {
	if (rPx <= 0.0) return 1.0;
	vec2 p = uv * sizePx;
	vec2 halfSz = sizePx * 0.5;
	vec2 q = abs(p - halfSz) - (halfSz - vec2(rPx));
	float d = length(max(q, vec2(0.0))) - rPx;
	return clamp(0.5 - d, 0.0, 1.0);
}

void main() {
	vec2 uv = u_uvRect.xy + v_uv * u_uvRect.zw;
	vec4 c = texture(u_tex, uv);
	float m = roundedMask(v_uv, u_sizePx, u_radiusPx);
	frag = vec4(c.rgb, c.a * u_alpha * m);
}`;

export interface QuadRect {
	x: number;
	y: number;
	w: number;
	h: number;
}

/** Texture sub-rect to sample (0..1): `u0,v0` origin + `du,dv` extent. `du`/`dv`
 *  may be negative to flip (mirror). Defaults to the full texture. */
export interface UvRect {
	u0: number;
	v0: number;
	du: number;
	dv: number;
}

export interface QuadDrawOptions {
	alpha?: number;
	/** Rounded-corner radius in px; `>= min(w,h)/2` yields a circle/stadium. */
	cornerRadiusPx?: number;
	/** Cover-crop / mirror the source into the rect; full texture when omitted. */
	uvRect?: UvRect;
}

export class OverlayQuad {
	#gl: WebGL2RenderingContext;
	#program: WebGLProgram;
	#buf: WebGLBuffer;
	#aQuad: number;
	#u: Record<string, WebGLUniformLocation | null>;

	private constructor(
		gl: WebGL2RenderingContext,
		program: WebGLProgram,
		buf: WebGLBuffer,
		aQuad: number,
		u: Record<string, WebGLUniformLocation | null>,
	) {
		this.#gl = gl;
		this.#program = program;
		this.#buf = buf;
		this.#aQuad = aQuad;
		this.#u = u;
	}

	static create(gl: WebGL2RenderingContext): OverlayQuad {
		const vs = compile(gl, gl.VERTEX_SHADER, VERT);
		const fs = compile(gl, gl.FRAGMENT_SHADER, FRAG);
		const program = link(gl, vs, fs);
		gl.deleteShader(vs);
		gl.deleteShader(fs);
		const buf = gl.createBuffer()!;
		gl.bindBuffer(gl.ARRAY_BUFFER, buf);
		gl.bufferData(
			gl.ARRAY_BUFFER,
			new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]),
			gl.STATIC_DRAW,
		);
		const aQuad = gl.getAttribLocation(program, "a_quad");
		const u: Record<string, WebGLUniformLocation | null> = {};
		for (const name of [
			"u_rect",
			"u_canvas",
			"u_tex",
			"u_alpha",
			"u_sizePx",
			"u_radiusPx",
			"u_uvRect",
		]) {
			u[name] = gl.getUniformLocation(program, name);
		}
		return new OverlayQuad(gl, program, buf, aQuad, u);
	}

	/** Blit `tex` into `rect` (pixel space) over the current framebuffer. Enables
	 *  straight-alpha blending for the draw and disables it after, so the main
	 *  pass stays blend-free. */
	draw(
		tex: WebGLTexture,
		rect: QuadRect,
		canvasW: number,
		canvasH: number,
		opts: QuadDrawOptions = {},
	): void {
		const gl = this.#gl;
		gl.useProgram(this.#program);
		gl.bindBuffer(gl.ARRAY_BUFFER, this.#buf);
		gl.enableVertexAttribArray(this.#aQuad);
		gl.vertexAttribPointer(this.#aQuad, 2, gl.FLOAT, false, 0, 0);

		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, tex);
		gl.uniform1i(this.#u.u_tex ?? null, 0);
		gl.uniform4f(this.#u.u_rect ?? null, rect.x, rect.y, rect.w, rect.h);
		gl.uniform2f(this.#u.u_canvas ?? null, canvasW, canvasH);
		gl.uniform1f(this.#u.u_alpha ?? null, opts.alpha ?? 1);
		gl.uniform2f(this.#u.u_sizePx ?? null, rect.w, rect.h);
		gl.uniform1f(this.#u.u_radiusPx ?? null, opts.cornerRadiusPx ?? 0);
		const uv = opts.uvRect;
		gl.uniform4f(this.#u.u_uvRect ?? null, uv?.u0 ?? 0, uv?.v0 ?? 0, uv?.du ?? 1, uv?.dv ?? 1);

		gl.enable(gl.BLEND);
		gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
		gl.drawArrays(gl.TRIANGLES, 0, 6);
		gl.disable(gl.BLEND);
	}

	dispose(): void {
		this.#gl.deleteBuffer(this.#buf);
		this.#gl.deleteProgram(this.#program);
	}
}
