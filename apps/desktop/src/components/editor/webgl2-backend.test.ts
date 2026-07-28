import { beforeEach, describe, expect, it, vi } from "vitest";
import { WebGL2Backend } from "./webgl2-backend";
import type { FrameUniforms } from "./frame-params";

const CONSTS = { VERTEX_SHADER: 1, FRAGMENT_SHADER: 2, COMPILE_STATUS: 3, LINK_STATUS: 4, ARRAY_BUFFER: 5, STATIC_DRAW: 6, FLOAT: 7, TRIANGLES: 8, TEXTURE1: 9, TEXTURE_2D: 10, COLOR_BUFFER_BIT: 11 };

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function fakeGl(): any {
	const gl: Record<string, unknown> = { ...CONSTS };
	for (const m of ["shaderSource", "compileShader", "deleteShader", "attachShader", "linkProgram", "deleteProgram", "bindBuffer", "bufferData", "enableVertexAttribArray", "vertexAttribPointer", "useProgram", "uniform1i", "uniform1f", "uniform2f", "uniform4fv", "uniform1fv", "viewport", "clearColor", "clear", "activeTexture", "bindTexture", "drawArrays"]) {
		gl[m] = vi.fn();
	}
	gl.createShader = vi.fn(() => ({}));
	gl.createProgram = vi.fn(() => ({}));
	gl.createBuffer = vi.fn(() => ({}));
	gl.getAttribLocation = vi.fn(() => 0);
	gl.getShaderParameter = vi.fn(() => true);
	gl.getProgramParameter = vi.fn(() => true);
	gl.getShaderInfoLog = vi.fn(() => "");
	gl.getProgramInfoLog = vi.fn(() => "");
	gl.getUniformLocation = vi.fn((_p: unknown, name: string) => ({ name }));
	return gl;
}

const UNIFORMS: FrameUniforms = {
	canvasSize: [800, 600], videoOrigin: [40, 30], videoSize: [720, 540], videoOpacity: 1, videoRotation: 0,
	bgType: 0, bgColor: [0.1, 0.1, 0.1, 1], gradColors: new Float32Array(32), gradStops: new Float32Array(8), gradCount: 0, gradAngle: 0, bgBlurPx: 0,
	zoomCenter: [0.5, 0.5], zoomScale: 1, motionBlurPx: 0, borderRadiusPx: 0,
	cursorPos: [0, 0], cursorVisible: 0, cursorRadius: 2, cursorColor: [1, 1, 1, 0.9], highlightColor: [0.2, 0.5, 0.9, 1], highlightAlpha: 0, highlightPos: [0, 0],
	shadowEnabled: 0, shadowBlurPx: 0, shadowSpreadPx: 0, shadowOffsetPx: [0, 0], shadowColor: [0, 0, 0, 0],
};

describe("WebGL2Backend.create", () => {
	const gl = fakeGl();
	WebGL2Backend.create(gl);

	it("looks up all 30 shader uniforms (drift guard vs the shader + applyFrameUniforms)", () => {
		const looked = gl.getUniformLocation.mock.calls.map((c: unknown[]) => c[1]);
		expect(looked.length).toBe(30);
		expect(looked).toContain("u_gradColors[0]");
		expect(looked).toContain("u_gradStops[0]");
		expect(looked).toContain("u_shadowColor");
	});

	it("binds the sampler units (video=0, background=1)", () => {
		expect(gl.uniform1i).toHaveBeenCalledWith({ name: "u_video" }, 0);
		expect(gl.uniform1i).toHaveBeenCalledWith({ name: "u_background" }, 1);
	});
});

describe("WebGL2Backend.beginFrame", () => {
	it("sets the viewport and clears to opaque black", () => {
		const gl = fakeGl();
		WebGL2Backend.create(gl).beginFrame(640, 480);
		expect(gl.viewport).toHaveBeenCalledWith(0, 0, 640, 480);
		expect(gl.clearColor).toHaveBeenCalledWith(0, 0, 0, 1);
		expect(gl.clear).toHaveBeenCalledWith(CONSTS.COLOR_BUFFER_BIT);
	});
});

describe("WebGL2Backend.renderMain", () => {
	let gl: ReturnType<typeof fakeGl>;
	let backend: WebGL2Backend;
	beforeEach(() => {
		gl = fakeGl();
		backend = WebGL2Backend.create(gl);
		for (const m of ["useProgram", "activeTexture", "bindTexture", "drawArrays", "uniform2f", "uniform1f", "uniform1i", "uniform4fv", "uniform1fv"]) gl[m].mockClear();
	});

	it("binds the background texture only when asked", () => {
		const bg = {} as WebGLTexture;
		backend.renderMain(UNIFORMS, { bindBackground: true, backgroundTex: bg });
		expect(gl.activeTexture).toHaveBeenCalledWith(CONSTS.TEXTURE1);
		expect(gl.bindTexture).toHaveBeenCalledWith(CONSTS.TEXTURE_2D, bg);
	});

	it("skips the background bind in color/gradient mode", () => {
		backend.renderMain(UNIFORMS, { bindBackground: false, backgroundTex: null });
		expect(gl.activeTexture).not.toHaveBeenCalled();
	});

	it("uses the program, writes the full uniform set, and draws the quad", () => {
		backend.renderMain(UNIFORMS, { bindBackground: false, backgroundTex: null });
		expect(gl.useProgram).toHaveBeenCalled();
		const writes = gl.uniform2f.mock.calls.length + gl.uniform1f.mock.calls.length + gl.uniform1i.mock.calls.length + gl.uniform4fv.mock.calls.length + gl.uniform1fv.mock.calls.length;
		expect(writes).toBe(28);
		expect(gl.drawArrays).toHaveBeenCalledWith(CONSTS.TRIANGLES, 0, 6);
	});
});
