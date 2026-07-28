/**
 * WebGL2 uniform-apply for the preview compositor: pushes a `FrameUniforms`
 * (from computeFrameParams) into the program's uniform locations. This is the
 * GL-backend half of the split — the pure value half is frame-params.ts. Keeping
 * them apart lets a WebGPU backend swap this for a uniform-buffer write while the
 * value computation stays shared.
 *
 * Every uniform is written every frame; the shader gates unused ones on
 * u_bgType/u_gradCount/u_shadowEnabled/etc., so a full write is visually
 * identical to the old conditional writes and avoids stale-state coupling.
 */

import type { FrameUniforms } from "./frame-params";

/** Cached `getUniformLocation` results, keyed by GLSL uniform name (array
 *  uniforms use their `name[0]` key, matching the shader + initGL). */
export type UniformLocations = Record<string, WebGLUniformLocation | null>;

export function applyFrameUniforms(
	gl: WebGL2RenderingContext,
	loc: UniformLocations,
	u: FrameUniforms,
): void {
	gl.uniform2f(loc.u_canvasSize, u.canvasSize[0], u.canvasSize[1]);
	gl.uniform2f(loc.u_videoOrigin, u.videoOrigin[0], u.videoOrigin[1]);
	gl.uniform2f(loc.u_videoSize, u.videoSize[0], u.videoSize[1]);
	gl.uniform1f(loc.u_videoOpacity, u.videoOpacity);
	gl.uniform1f(loc.u_videoRotation, u.videoRotation);

	gl.uniform1i(loc.u_bgType, u.bgType);
	gl.uniform4fv(loc.u_bgColor, u.bgColor);
	gl.uniform4fv(loc["u_gradColors[0]"], u.gradColors);
	gl.uniform1fv(loc["u_gradStops[0]"], u.gradStops);
	gl.uniform1i(loc.u_gradCount, u.gradCount);
	gl.uniform1f(loc.u_gradAngle, u.gradAngle);
	gl.uniform1f(loc.u_bgBlurPx, u.bgBlurPx);

	gl.uniform2f(loc.u_zoomCenter, u.zoomCenter[0], u.zoomCenter[1]);
	gl.uniform1f(loc.u_zoomScale, u.zoomScale);
	gl.uniform1f(loc.u_motionBlurPx, u.motionBlurPx);
	gl.uniform1f(loc.u_borderRadiusPx, u.borderRadiusPx);

	gl.uniform2f(loc.u_cursorPos, u.cursorPos[0], u.cursorPos[1]);
	gl.uniform1f(loc.u_cursorVisible, u.cursorVisible);
	gl.uniform1f(loc.u_cursorRadius, u.cursorRadius);
	gl.uniform4fv(loc.u_cursorColor, u.cursorColor);
	gl.uniform4fv(loc.u_highlightColor, u.highlightColor);
	gl.uniform1f(loc.u_highlightAlpha, u.highlightAlpha);
	gl.uniform2f(loc.u_highlightPos, u.highlightPos[0], u.highlightPos[1]);

	gl.uniform1i(loc.u_shadowEnabled, u.shadowEnabled);
	gl.uniform1f(loc.u_shadowBlurPx, u.shadowBlurPx);
	gl.uniform1f(loc.u_shadowSpreadPx, u.shadowSpreadPx);
	gl.uniform2f(loc.u_shadowOffsetPx, u.shadowOffsetPx[0], u.shadowOffsetPx[1]);
	gl.uniform4fv(loc.u_shadowColor, u.shadowColor);
}
