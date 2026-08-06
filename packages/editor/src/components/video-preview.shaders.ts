/**
 * GLSL sources for the editor preview compositor. Kept as plain strings so the
 * shader logic (background gradient, rounded-rect mask, zoom, motion blur,
 * cursor + click highlight, drop shadow) lives in one place. Several branches
 * mirror the Rust export rasteriser 1:1. Keep the two in lockstep.
 */

export const VERT_SRC = `#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
	v_uv = a_pos * 0.5 + 0.5;
	v_uv.y = 1.0 - v_uv.y;
	gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

export const FRAG_SRC = `#version 300 es
precision highp float;

uniform sampler2D u_video;
uniform sampler2D u_background;

uniform vec2 u_canvasSize;        // pixels
// Source-video rectangle inside the canvas. Replaces the v1 single
// u_paddingPx so we can letterbox/pillarbox to a target aspect ratio
// (the bars between the comp and the canvas edge are filled by the
// background).
uniform vec2 u_videoOrigin;       // pixels: top-left of source video
uniform vec2 u_videoSize;         // pixels: source video w/h
uniform int u_bgType;             // 0=color, 1=gradient, 2=image
uniform vec4 u_bgColor;           // [0..1]
// Multi-stop linear gradient. Colors + their positions (0..1) along the
// gradient line, plus the stop count and the CSS angle (radians). MAX_STOPS
// mirrors MAX_GRADIENT_STOPS in the store + the Rust export rasteriser.
#define MAX_STOPS 8
uniform vec4 u_gradColors[MAX_STOPS];
uniform float u_gradStops[MAX_STOPS];
uniform int u_gradCount;
uniform float u_gradAngle;        // radians (CSS convention: 0 = up, CW)
uniform float u_bgBlurPx;         // image-mode blur radius in canvas pixels (0 = off)
uniform vec2 u_zoomCenter;        // [0..1] in video UV
uniform float u_zoomScale;        // 1.0 = no zoom
uniform float u_motionBlurPx;     // radial motion-blur radius in canvas px (0 = off)
uniform float u_borderRadiusPx;   // rounded corner radius of the video rect, canvas pixels
uniform float u_videoOpacity;     // scene entrance/exit fade on the video layer (1 = opaque)
uniform float u_videoRotation;    // scene rotation of the video card, radians about its centre

uniform vec2 u_cursorPos;         // [0..1] in video UV
uniform float u_cursorVisible;    // 0 or 1
uniform float u_cursorRadius;     // pixels (canvas)
uniform vec4 u_cursorColor;
uniform vec4 u_highlightColor;
uniform float u_highlightAlpha;   // 0 if no click highlight
uniform vec2 u_highlightPos;      // [0..1] video UV, ALREADY zoom-transformed: the
                                  // captured click point, independent of the cursor

// Drop shadow cast by the video rect onto the background.
uniform int u_shadowEnabled;      // 0 / 1
uniform float u_shadowBlurPx;     // soft edge width
uniform float u_shadowSpreadPx;   // rect grows by this much before blur
uniform vec2 u_shadowOffsetPx;    // (x, y) offset
uniform vec4 u_shadowColor;       // rgb + alpha

in vec2 v_uv;
out vec4 frag;

vec4 sampleBackground(vec2 uv) {
	if (u_bgType == 0) return u_bgColor;
	if (u_bgType == 1) {
		// Multi-stop linear gradient with a real CSS angle. Project the pixel
		// onto the gradient line in PIXEL space (aspect-aware) so the visual
		// angle matches the picker swatch and the exported PNG exactly. The
		// Rust rasteriser uses identical math. Keep the two in lockstep.
		vec2 dir = vec2(sin(u_gradAngle), -cos(u_gradAngle));
		vec2 p = (uv - 0.5) * u_canvasSize;
		float ext = abs(dir.x) * u_canvasSize.x + abs(dir.y) * u_canvasSize.y;
		float t = clamp(0.5 + dot(p, dir) / max(ext, 1.0), 0.0, 1.0);
		// Walk the stops; the highest stop whose position is <= t owns the
		// segment, so the final assignment is the correct interpolation.
		vec4 col = u_gradColors[0];
		for (int i = 0; i < MAX_STOPS - 1; i++) {
			if (i + 1 >= u_gradCount) break;
			float a = u_gradStops[i];
			float b = u_gradStops[i + 1];
			if (t >= a) {
				float seg = clamp((t - a) / max(b - a, 1e-5), 0.0, 1.0);
				col = mix(u_gradColors[i], u_gradColors[i + 1], seg);
			}
		}
		return col;
	}
	// Image / wallpaper, optionally blurred with a cheap separable-ish 9-tap kernel.
	if (u_bgBlurPx <= 0.5) {
		return texture(u_background, uv);
	}
	// Multi-tap gaussian approximation: 9 samples in a diamond/cross pattern
	// with radius in UV space. Good enough for background blur at small
	// radii; heavier blur is faked by larger step and stronger weights.
	vec2 step = vec2(u_bgBlurPx, u_bgBlurPx) / u_canvasSize;
	vec4 c = vec4(0.0);
	c += texture(u_background, uv) * 0.227027;
	c += texture(u_background, uv + vec2( step.x,  0.0)) * 0.1945946;
	c += texture(u_background, uv + vec2(-step.x,  0.0)) * 0.1945946;
	c += texture(u_background, uv + vec2( 0.0,  step.y)) * 0.1216216;
	c += texture(u_background, uv + vec2( 0.0, -step.y)) * 0.1216216;
	c += texture(u_background, uv + vec2( step.x * 2.0,  0.0)) * 0.054054;
	c += texture(u_background, uv + vec2(-step.x * 2.0,  0.0)) * 0.054054;
	c += texture(u_background, uv + vec2( 0.0,  step.y * 2.0)) * 0.054054;
	c += texture(u_background, uv + vec2( 0.0, -step.y * 2.0)) * 0.054054;
	return c;
}

// Signed distance from 'p' to a centered rounded rect of half-size 'hs' and radius 'r'.
// Negative inside, positive outside.
float sdRoundRect(vec2 p, vec2 hs, float r) {
	vec2 q = abs(p) - hs + vec2(r);
	return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
	vec2 canvasPx = v_uv * u_canvasSize;

	vec2 videoMin = u_videoOrigin;
	vec2 videoMax = u_videoOrigin + u_videoSize;
	vec2 videoSize = max(u_videoSize, vec2(1.0));

	vec4 color = sampleBackground(v_uv);

	// Rounded-rect mask for the video region.
	vec2 videoCenter = (videoMin + videoMax) * 0.5;
	// Scene rotation: spin the whole card about its centre by inverse-rotating the
	// sampling coordinate: the mask, video UV, cursor and highlight all derive
	// from canvasPx, so rotating it here rotates the card as one.
	if (abs(u_videoRotation) > 0.0001) {
		float rs = sin(u_videoRotation);
		float rc = cos(u_videoRotation);
		vec2 rd = canvasPx - videoCenter;
		canvasPx = videoCenter + vec2(rc * rd.x + rs * rd.y, -rs * rd.x + rc * rd.y);
	}
	vec2 halfSize = videoSize * 0.5;
	// Clamp radius so it never exceeds half the smaller dimension.
	float maxR = min(halfSize.x, halfSize.y);
	float r = clamp(u_borderRadiusPx, 0.0, maxR);
	float sd = sdRoundRect(canvasPx - videoCenter, halfSize, r);
	// Coverage = 1 inside, fading to 0 over ~1 px at the edge for AA.
	float videoCoverage = 1.0 - smoothstep(-1.0, 0.0, sd);

	// Drop shadow, computed before the video mix so it sits under the rect.
	// Reuse sdRoundRect against an offset, spread-expanded clone of the video
	// rectangle, then falls off across u_shadowBlurPx pixels.
	if (u_shadowEnabled == 1 && u_shadowColor.a > 0.0) {
		float spread = max(u_shadowSpreadPx, 0.0);
		float blurPx = max(u_shadowBlurPx, 0.5);
		vec2 shadowP = (canvasPx - videoCenter) - u_shadowOffsetPx;
		float sdShadow = sdRoundRect(shadowP, halfSize + vec2(spread), r + spread * 0.5);
		float shadowMask = 1.0 - smoothstep(0.0, blurPx, sdShadow);
		// Don't bleed shadow onto the video surface.
		shadowMask *= (1.0 - videoCoverage);
		// Fade the shadow with the video layer so the whole card animates as one.
		color.rgb = mix(color.rgb, u_shadowColor.rgb, shadowMask * u_shadowColor.a * u_videoOpacity);
	}

	if (videoCoverage > 0.0) {
		vec2 videoUV = (canvasPx - videoMin) / videoSize;

		// Apply zoom: shrink uv toward zoom center
		if (u_zoomScale > 1.0001) {
			videoUV = (videoUV - u_zoomCenter) / u_zoomScale + u_zoomCenter;
			videoUV = clamp(videoUV, 0.0, 1.0);
		}

		// Radial motion blur centred on the focus point. Direction = vector
		// from zoom centre outward; magnitude driven by d(scale)/dt in JS.
		// 13 taps with a triangular weight so a strong dolly smear stays smooth.
		vec4 videoColor;
		if (u_motionBlurPx > 0.5) {
			vec2 dir = (videoUV - u_zoomCenter) * (u_motionBlurPx / max(u_canvasSize.x, 1.0));
			vec4 acc = vec4(0.0);
			float w = 0.0;
			for (int i = -6; i <= 6; i++) {
				float fi = float(i) / 6.0;
				vec2 uv = clamp(videoUV + dir * fi, 0.0, 1.0);
				float wi = 1.0 - abs(fi) * 0.5;
				acc += texture(u_video, uv) * wi;
				w += wi;
			}
			videoColor = acc / w;
		} else {
			videoColor = texture(u_video, videoUV);
		}

		// Click highlight halo, PINNED to the captured click point
		// (u_highlightPos, already zoom-transformed), drawn under the cursor and
		// independent of the cursor sprite / its visibility. This is what makes
		// the ring land exactly where AND when the click happened even with
		// smoothing on (the smoothed cursor lags, so riding it read as delayed,
		// off-target feedback). u_highlightPos already carries the same affine
		// zoom as the cursor, so it tracks the zoomed video.
		if (u_highlightAlpha > 0.0) {
			vec2 hlUV = u_highlightPos;
			if (hlUV.x >= 0.0 && hlUV.x <= 1.0 && hlUV.y >= 0.0 && hlUV.y <= 1.0) {
				vec2 hlPx = videoMin + hlUV * videoSize;
				float hdist = length(canvasPx - hlPx);
				float hr = u_cursorRadius * 6.0;
				float ha = (1.0 - smoothstep(hr - 4.0, hr, hdist)) * u_highlightAlpha;
				videoColor = mix(videoColor, u_highlightColor, ha);
			}
		}

		// Cursor overlay (drawn on top of video, clipped to rounded video region).
		if (u_cursorVisible > 0.5) {
			vec2 cursorUV = u_cursorPos;
			if (u_zoomScale > 1.0001) {
				cursorUV = (cursorUV - u_zoomCenter) * u_zoomScale + u_zoomCenter;
			}

			if (cursorUV.x >= 0.0 && cursorUV.x <= 1.0 && cursorUV.y >= 0.0 && cursorUV.y <= 1.0) {
				vec2 cursorPx = videoMin + cursorUV * videoSize;
				float dist = length(canvasPx - cursorPx);

				float cd = 1.0 - smoothstep(u_cursorRadius - 1.5, u_cursorRadius, dist);
				videoColor = mix(videoColor, u_cursorColor, cd * u_cursorColor.a);
			}
		}

		// Mix the composed video (+cursor) over the background using the rounded mask.
		color = mix(color, videoColor, videoCoverage * u_videoOpacity);
	}

	frag = vec4(color.rgb, 1.0);
}`;
