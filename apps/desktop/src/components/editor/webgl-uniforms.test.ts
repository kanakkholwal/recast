import { describe, expect, it, vi } from "vitest";
import { applyFrameUniforms, type UniformLocations } from "./webgl-uniforms";
import type { FrameUniforms } from "./frame-params";

function fakeGl() {
	return {
		uniform1i: vi.fn(),
		uniform1f: vi.fn(),
		uniform2f: vi.fn(),
		uniform4fv: vi.fn(),
		uniform1fv: vi.fn(),
	};
}

// Sentinel location per name so we can assert each uniform hit its own slot.
function locations(names: string[]): UniformLocations {
	const loc: UniformLocations = {};
	for (const n of names) loc[n] = { name: n } as unknown as WebGLUniformLocation;
	return loc;
}

const UNIFORMS: FrameUniforms = {
	canvasSize: [1920, 1080],
	videoOrigin: [100, 50],
	videoSize: [1720, 980],
	videoOpacity: 0.5,
	videoRotation: 0.25,
	bgType: 1,
	bgColor: [0.1, 0.2, 0.3, 1],
	gradColors: new Float32Array(32).fill(0.4),
	gradStops: new Float32Array(8).fill(0.6),
	gradCount: 3,
	gradAngle: 1.5,
	bgBlurPx: 12,
	zoomCenter: [0.3, 0.7],
	zoomScale: 2,
	motionBlurPx: 4,
	borderRadiusPx: 108,
	cursorPos: [0.5, 0.5],
	cursorVisible: 1,
	cursorRadius: 6,
	cursorColor: [1, 1, 1, 0.9],
	highlightColor: [0.2, 0.5, 0.9, 1],
	highlightAlpha: 0.8,
	highlightPos: [0.4, 0.6],
	shadowEnabled: 1,
	shadowBlurPx: 20,
	shadowSpreadPx: 4,
	shadowOffsetPx: [0, 8],
	shadowColor: [0, 0, 0, 0.5],
};

describe("applyFrameUniforms", () => {
	const gl = fakeGl();
	const loc = locations([
		"u_canvasSize", "u_videoOrigin", "u_videoSize", "u_videoOpacity", "u_videoRotation",
		"u_bgType", "u_bgColor", "u_gradColors[0]", "u_gradStops[0]", "u_gradCount", "u_gradAngle", "u_bgBlurPx",
		"u_zoomCenter", "u_zoomScale", "u_motionBlurPx", "u_borderRadiusPx",
		"u_cursorPos", "u_cursorVisible", "u_cursorRadius", "u_cursorColor", "u_highlightColor", "u_highlightAlpha", "u_highlightPos",
		"u_shadowEnabled", "u_shadowBlurPx", "u_shadowSpreadPx", "u_shadowOffsetPx", "u_shadowColor",
	]);
	applyFrameUniforms(gl as unknown as WebGL2RenderingContext, loc, UNIFORMS);

	it("writes vec2 uniforms to their own locations with both components", () => {
		expect(gl.uniform2f).toHaveBeenCalledWith(loc.u_canvasSize, 1920, 1080);
		expect(gl.uniform2f).toHaveBeenCalledWith(loc.u_videoOrigin, 100, 50);
		expect(gl.uniform2f).toHaveBeenCalledWith(loc.u_zoomCenter, 0.3, 0.7);
		expect(gl.uniform2f).toHaveBeenCalledWith(loc.u_shadowOffsetPx, 0, 8);
	});

	it("writes int uniforms via uniform1i", () => {
		expect(gl.uniform1i).toHaveBeenCalledWith(loc.u_bgType, 1);
		expect(gl.uniform1i).toHaveBeenCalledWith(loc.u_gradCount, 3);
		expect(gl.uniform1i).toHaveBeenCalledWith(loc.u_shadowEnabled, 1);
	});

	it("writes the gradient arrays to their [0] locations", () => {
		expect(gl.uniform4fv).toHaveBeenCalledWith(loc["u_gradColors[0]"], UNIFORMS.gradColors);
		expect(gl.uniform1fv).toHaveBeenCalledWith(loc["u_gradStops[0]"], UNIFORMS.gradStops);
	});

	it("writes float + vec4 scalars", () => {
		expect(gl.uniform1f).toHaveBeenCalledWith(loc.u_zoomScale, 2);
		expect(gl.uniform1f).toHaveBeenCalledWith(loc.u_borderRadiusPx, 108);
		expect(gl.uniform4fv).toHaveBeenCalledWith(loc.u_bgColor, UNIFORMS.bgColor);
		expect(gl.uniform4fv).toHaveBeenCalledWith(loc.u_shadowColor, UNIFORMS.shadowColor);
	});

	it("writes the full uniform set (no stale-state reliance)", () => {
		const total =
			gl.uniform1i.mock.calls.length +
			gl.uniform1f.mock.calls.length +
			gl.uniform2f.mock.calls.length +
			gl.uniform4fv.mock.calls.length +
			gl.uniform1fv.mock.calls.length;
		expect(total).toBe(28);
	});
});
