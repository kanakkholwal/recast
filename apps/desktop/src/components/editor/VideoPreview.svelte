<script lang="ts">
	import { resolveAsset } from "$lib/assets";
	import {
	  smoothCursorPath,
	  smoothingStrengthToSigmaMs,
	} from "$lib/cursor/smoothing";
	import { CURSOR_STYLES, cursorStyleDataUrl } from "$lib/cursor/styles";
	import { bezierY } from "$lib/easing/cubic-bezier";
	import { assetsStore } from "$lib/stores/assets-store.svelte";
	import {
	  framePaddingPixels,
	  type EditorStore,
	} from "$lib/stores/editor-store.svelte";
	import { Spinner } from "@recast/ui/spinner";
	import { convertFileSrc } from "@tauri-apps/api/core";
	import { onDestroy, onMount } from "svelte";
	import AnnotationOverlay from "./_components/AnnotationOverlay.svelte";
	import FocusOverlay from "./_components/FocusOverlay.svelte";
	import TextAnnotationLayer from "./_components/TextAnnotationLayer.svelte";

	interface Props {
		store: EditorStore;
		videoEl: HTMLVideoElement | null;
		videoSrc: string;
		cursorPath: string | null;
		onTimeUpdate: () => void;
		onEnded: () => void;
		onLoadedMetadata: () => void;
		onReady: () => void;
		onError: () => void;
		onSeeked?: () => void;
	}

	let {
		store,
		videoEl = $bindable(null),
		videoSrc,
		cursorPath,
		onTimeUpdate,
		onEnded,
		onLoadedMetadata,
		onReady,
		onError,
		onSeeked,
	}: Props = $props();

	//  DOM refs & GL state 
	let canvasEl: HTMLCanvasElement | null = $state(null);
	let containerEl: HTMLDivElement | null = $state(null);
	/** Shrink-wrap around the WebGL canvas so the annotation overlay can sit
	 * on top of it at the same rendered rect regardless of letterboxing. */
	let previewRectEl: HTMLDivElement | null = $state(null);
	let isReady = $state(false);

	let gl: WebGL2RenderingContext | null = null;
	let program: WebGLProgram | null = null;
	let videoTex: WebGLTexture | null = null;
	let bgTex: WebGLTexture | null = null;
	let bgTexReady = false;
	let lastBgKey = "";

	// Uniform locations
	const uniforms: Record<string, WebGLUniformLocation | null> = {};

	// rVFC handle for playback redraw
	let rvfcHandle: number | null = null;
	// RAF handle for coalescing reactive redraws
	let rafHandle: number | null = null;

	// Cursor track
	type CursorSampleJS = {
		timestampUs: number;
		x: number;
		y: number;
		visible: boolean;
		leftDown: boolean;
		rightDown: boolean;
	};
	type IdlePeriodJS = { startUs: number; endUs: number };
	let cursorSamplesRaw: CursorSampleJS[] = [];
	let cursorSamples: CursorSampleJS[] = []; // post-smoothing; read by interpolateCursor
	let idlePeriods: IdlePeriodJS[] = [];
	let loadedCursorPath = "";

	// SVG cursor overlay state. Updated each draw() call when the user has
	// picked a non-`dot` cursor style; consumed by the absolutely-positioned
	// <img> at the bottom of the markup. Not a $derived — the data is pulled
	// from the WebGL draw loop where the cursor sample is already evaluated.
	let svgCursor = $state<{
		visible: boolean;
		alpha: number;
		styleId: import("$lib/cursor/styles").CursorStyle["id"];
		pressed: boolean;
		canvasX: number; // source-pixel space, includes padding offset
		canvasY: number;
		compW: number;
		compH: number;
		spritePx: number; // sprite size in source pixels — render width = (spritePx/compW)*100%
	}>({
		visible: false,
		alpha: 0,
		styleId: "dot",
		pressed: false,
		canvasX: 0,
		canvasY: 0,
		compW: 1,
		compH: 1,
		spritePx: 32,
	});
	// Signature of the inputs that drive smoothing. Recomputing only when this
	// changes keeps playback cheap even on long recordings.
	let smoothingSignature = "";

	//  Shaders 
	const VERT_SRC = `#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
	v_uv = a_pos * 0.5 + 0.5;
	v_uv.y = 1.0 - v_uv.y;
	gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

	const FRAG_SRC = `#version 300 es
precision highp float;

uniform sampler2D u_video;
uniform sampler2D u_background;

uniform vec2 u_canvasSize;        // pixels
uniform float u_paddingPx;        // pixels of padding inside canvas
uniform int u_bgType;             // 0=color, 1=gradient, 2=image
uniform vec4 u_bgColor;           // [0..1]
uniform vec4 u_gradStart;
uniform vec4 u_gradEnd;
uniform float u_bgBlurPx;         // image-mode blur radius in canvas pixels (0 = off)
uniform vec2 u_zoomCenter;        // [0..1] in video UV
uniform float u_zoomScale;        // 1.0 = no zoom
uniform float u_motionBlurPx;     // radial motion-blur radius in canvas px (0 = off)
uniform float u_borderRadiusPx;   // rounded corner radius of the video rect, canvas pixels

uniform vec2 u_cursorPos;         // [0..1] in video UV
uniform float u_cursorVisible;    // 0 or 1
uniform float u_cursorRadius;     // pixels (canvas)
uniform vec4 u_cursorColor;
uniform vec4 u_highlightColor;
uniform float u_highlightAlpha;   // 0 if no click highlight

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
		// Diagonal gradient (matches the 135deg gradient presets in the store)
		float t = clamp((uv.x + uv.y) * 0.5, 0.0, 1.0);
		return mix(u_gradStart, u_gradEnd, t);
	}
	// Image / wallpaper — optionally blurred with a cheap separable-ish 9-tap kernel.
	if (u_bgBlurPx <= 0.5) {
		return texture(u_background, uv);
	}
	// Multi-tap gaussian approximation — 9 samples in a diamond/cross pattern
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

	vec2 videoMin = vec2(u_paddingPx);
	vec2 videoMax = u_canvasSize - vec2(u_paddingPx);
	vec2 videoSize = max(videoMax - videoMin, vec2(1.0));

	vec4 color = sampleBackground(v_uv);

	// Rounded-rect mask for the video region.
	vec2 videoCenter = (videoMin + videoMax) * 0.5;
	vec2 halfSize = videoSize * 0.5;
	// Clamp radius so it never exceeds half the smaller dimension.
	float maxR = min(halfSize.x, halfSize.y);
	float r = clamp(u_borderRadiusPx, 0.0, maxR);
	float sd = sdRoundRect(canvasPx - videoCenter, halfSize, r);
	// Coverage = 1 inside, fading to 0 over ~1 px at the edge for AA.
	float videoCoverage = 1.0 - smoothstep(-1.0, 0.0, sd);

	// Drop shadow — computed before the video mix so it sits under the rect.
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
		color.rgb = mix(color.rgb, u_shadowColor.rgb, shadowMask * u_shadowColor.a);
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
		// 7 taps with a triangular weight — cheap enough per fragment.
		vec4 videoColor;
		if (u_motionBlurPx > 0.5) {
			vec2 dir = (videoUV - u_zoomCenter) * (u_motionBlurPx / max(u_canvasSize.x, 1.0));
			vec4 acc = vec4(0.0);
			float w = 0.0;
			for (int i = -3; i <= 3; i++) {
				float fi = float(i) / 3.0;
				vec2 uv = clamp(videoUV + dir * fi, 0.0, 1.0);
				float wi = 1.0 - abs(fi) * 0.5;
				acc += texture(u_video, uv) * wi;
				w += wi;
			}
			videoColor = acc / w;
		} else {
			videoColor = texture(u_video, videoUV);
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

				if (u_highlightAlpha > 0.0) {
					float hr = u_cursorRadius * 6.0;
					float ha = (1.0 - smoothstep(hr - 4.0, hr, dist)) * u_highlightAlpha;
					videoColor = mix(videoColor, u_highlightColor, ha);
				}

				float cd = 1.0 - smoothstep(u_cursorRadius - 1.5, u_cursorRadius, dist);
				videoColor = mix(videoColor, u_cursorColor, cd * u_cursorColor.a);
			}
		}

		// Mix the composed video (+cursor) over the background using the rounded mask.
		color = mix(color, videoColor, videoCoverage);
	}

	frag = vec4(color.rgb, 1.0);
}`;

	//  GL helpers 
	function compile(g: WebGL2RenderingContext, type: number, src: string): WebGLShader {
		const sh = g.createShader(type)!;
		g.shaderSource(sh, src);
		g.compileShader(sh);
		if (!g.getShaderParameter(sh, g.COMPILE_STATUS)) {
			const log = g.getShaderInfoLog(sh);
			g.deleteShader(sh);
			throw new Error(`Shader compile failed: ${log}`);
		}
		return sh;
	}

	function link(g: WebGL2RenderingContext, vs: WebGLShader, fs: WebGLShader): WebGLProgram {
		const p = g.createProgram()!;
		g.attachShader(p, vs);
		g.attachShader(p, fs);
		g.linkProgram(p);
		if (!g.getProgramParameter(p, g.LINK_STATUS)) {
			const log = g.getProgramInfoLog(p);
			g.deleteProgram(p);
			throw new Error(`Program link failed: ${log}`);
		}
		return p;
	}

	function initGL() {
		if (!canvasEl) return;
		const g = canvasEl.getContext("webgl2", {
			alpha: false,
			antialias: false,
			premultipliedAlpha: false,
			preserveDrawingBuffer: false,
		});
		if (!g) {
			console.error("WebGL2 not supported in this WebView");
			return;
		}
		gl = g;

		const vs = compile(g, g.VERTEX_SHADER, VERT_SRC);
		const fs = compile(g, g.FRAGMENT_SHADER, FRAG_SRC);
		program = link(g, vs, fs);
		g.deleteShader(vs);
		g.deleteShader(fs);

		// Full-screen quad
		const buf = g.createBuffer();
		g.bindBuffer(g.ARRAY_BUFFER, buf);
		g.bufferData(
			g.ARRAY_BUFFER,
			new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
			g.STATIC_DRAW,
		);
		const aPos = g.getAttribLocation(program, "a_pos");
		g.enableVertexAttribArray(aPos);
		g.vertexAttribPointer(aPos, 2, g.FLOAT, false, 0, 0);

		// Cache uniform locations
		for (const name of [
			"u_video",
			"u_background",
			"u_canvasSize",
			"u_paddingPx",
			"u_bgType",
			"u_bgColor",
			"u_gradStart",
			"u_gradEnd",
			"u_bgBlurPx",
			"u_zoomCenter",
			"u_zoomScale",
			"u_motionBlurPx",
			"u_borderRadiusPx",
			"u_cursorPos",
			"u_cursorVisible",
			"u_cursorRadius",
			"u_cursorColor",
			"u_highlightColor",
			"u_highlightAlpha",
			"u_shadowEnabled",
			"u_shadowBlurPx",
			"u_shadowSpreadPx",
			"u_shadowOffsetPx",
			"u_shadowColor",
		]) {
			uniforms[name] = g.getUniformLocation(program, name);
		}

		// Allocate textures
		videoTex = g.createTexture();
		g.bindTexture(g.TEXTURE_2D, videoTex);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_S, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_T, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MIN_FILTER, g.LINEAR);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MAG_FILTER, g.LINEAR);

		bgTex = g.createTexture();
		g.bindTexture(g.TEXTURE_2D, bgTex);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_S, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_WRAP_T, g.CLAMP_TO_EDGE);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MIN_FILTER, g.LINEAR);
		g.texParameteri(g.TEXTURE_2D, g.TEXTURE_MAG_FILTER, g.LINEAR);
		// Placeholder 1×1 transparent texture so the sampler is always valid
		g.texImage2D(g.TEXTURE_2D, 0, g.RGBA, 1, 1, 0, g.RGBA, g.UNSIGNED_BYTE, new Uint8Array([0, 0, 0, 0]));

		g.useProgram(program);
		g.uniform1i(uniforms.u_video, 0);
		g.uniform1i(uniforms.u_background, 1);
	}

	//  Background loading 
	async function resolveBackgroundSrc(value: string): Promise<string> {
		if (!value) return "";
		// Defensive: gradient/colour values must never reach convertFileSrc —
		// the caller already gates on backgroundType, but a stray write that
		// leaves a CSS gradient in backgroundValue while type briefly reads
		// "image" would otherwise log "File does not exist at path: linear-
		// gradient(...)" via the Tauri asset protocol.
		if (value.includes("gradient(") || value.startsWith("#")) return "";
		if (value.startsWith("asset:") && !value.startsWith("asset://")) {
			const id = value.slice("asset:".length);
			const cached = await resolveAsset(id);
			if (cached) return convertFileSrc(cached);
			const thumb = assetsStore.thumbPaths[id];
			if (thumb) return convertFileSrc(thumb);
			return "";
		}
		if (
			value.startsWith("data:") ||
			value.startsWith("http://") ||
			value.startsWith("https://") ||
			value.startsWith("asset://") ||
			value.startsWith("/")
		) {
			// Already a URL (served or data) or a root-relative path served
			// from the frontend's static/ dir.
			return value;
		}
		// Raw filesystem path — convert to the Tauri asset protocol.
		return convertFileSrc(value);
	}

	async function loadBackgroundIfNeeded() {
		if (!gl || !bgTex) return;
		const type = store.backgroundType;
		const value = store.backgroundValue;
		// Including the resolved cache path in the key ensures the texture
		// re-loads when an `asset:<id>` download lands after an initial miss,
		// or when the thumbnail lands before the full-res does.
		let resolvedForKey = value;
		if (value.startsWith("asset:") && !value.startsWith("asset://")) {
			const id = value.slice("asset:".length);
			resolvedForKey =
				assetsStore.paths[id] ?? assetsStore.thumbPaths[id] ?? value;
		}
		const key = `${type}|${resolvedForKey}`;
		if (key === lastBgKey) return;
		lastBgKey = key;

		if (type !== "wallpaper" && type !== "image") {
			bgTexReady = false;
			return;
		}

		if (!value) {
			bgTexReady = false;
			return;
		}

		try {
			const resolvedSrc = await resolveBackgroundSrc(value);
			if (!resolvedSrc) {
				// Asset not yet cached (first-run offline, or still downloading).
				// Fall through to flat-background rendering until a later tick
				// re-runs this effect once the cache populates.
				bgTexReady = false;
				return;
			}
			const img = new Image();
			img.crossOrigin = "anonymous";
			img.src = resolvedSrc;
			await img.decode();
			if (lastBgKey !== key) return; // Superseded by another load
			gl.bindTexture(gl.TEXTURE_2D, bgTex);
			gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
			bgTexReady = true;
			requestRedraw();
		} catch (err) {
			console.warn("Background image load failed:", err, "value:", value);
			bgTexReady = false;
		}
	}

	//  Cursor track loading 
	async function loadCursorTrackIfNeeded() {
		if (!cursorPath || cursorPath === loadedCursorPath) return;
		try {
			const url = convertFileSrc(cursorPath);
			const res = await fetch(url);
			if (!res.ok) throw new Error(`HTTP ${res.status}`);
			const json = (await res.json()) as {
				samples?: CursorSampleJS[];
				idlePeriods?: IdlePeriodJS[];
			};
			cursorSamplesRaw = json.samples ?? [];
			cursorSamples = cursorSamplesRaw;
			idlePeriods = json.idlePeriods ?? [];
			loadedCursorPath = cursorPath;
			smoothingSignature = "";
			// Publish raw samples for the Cursor panel's trajectory minimap.
			store.cursorSamplesRaw = cursorSamplesRaw;
			ensureSmoothingCurrent();
		} catch (err) {
			console.warn("Cursor track load failed:", err);
			cursorSamplesRaw = [];
			cursorSamples = [];
			idlePeriods = [];
		}
	}

	// Recompute the smoothed cursor path whenever the inputs change. Called
	// once per draw() — cheap signature check, real work only on deltas.
	function ensureSmoothingCurrent() {
		if (cursorSamplesRaw.length === 0) {
			cursorSamples = cursorSamplesRaw;
			smoothingSignature = "";
			return;
		}
		const cs = store.cursorSettings;
		const sig = `${loadedCursorPath}|${cs.smoothing}|${cs.snapToClicks ? 1 : 0}|${cs.snapWindowMs}`;
		if (sig === smoothingSignature) return;
		const sigmaMs = smoothingStrengthToSigmaMs(cs.smoothing);
		const result = smoothCursorPath(cursorSamplesRaw, {
			sigmaMs,
			snapToClicks: cs.snapToClicks,
			snapWindowMs: cs.snapWindowMs,
		});
		cursorSamples = result.samples;
		smoothingSignature = sig;
	}

	// Idle hide fade — shared 200ms ramp at each end of an idle period.
	// Mirrored 1:1 in `cursor_export.rs` so preview and export agree.
	const CURSOR_IDLE_FADE_US = 200_000;
	function idleAlphaAt(tsUs: number, idleTimeoutSec: number): number {
		const thresholdUs = idleTimeoutSec * 1_000_000;
		for (const period of idlePeriods) {
			const fadeStart = period.startUs + thresholdUs;
			if (period.endUs <= fadeStart) continue;
			const fadeEnd = Math.min(fadeStart + CURSOR_IDLE_FADE_US, period.endUs);
			const resumeStart = Math.max(period.endUs - CURSOR_IDLE_FADE_US, fadeEnd);
			if (tsUs < fadeStart || tsUs > period.endUs) continue;
			if (tsUs >= fadeEnd && tsUs <= resumeStart) return 0;
			if (tsUs < fadeEnd) {
				return 1 - (tsUs - fadeStart) / (fadeEnd - fadeStart);
			}
			return 1 - (period.endUs - tsUs) / (period.endUs - resumeStart);
		}
		return 1;
	}

	//  Cursor interpolation (mirror of cursor::smoothing::interpolate_at)
	function interpolateCursor(timestampUs: number) {
		if (cursorSamples.length === 0) return null;
		// Binary search
		let lo = 0;
		let hi = cursorSamples.length;
		while (lo < hi) {
			const mid = (lo + hi) >>> 1;
			if (cursorSamples[mid].timestampUs < timestampUs) lo = mid + 1;
			else hi = mid;
		}
		const idx = lo;
		if (idx >= cursorSamples.length) return cursorSamples[cursorSamples.length - 1];
		if (idx === 0 || cursorSamples[idx].timestampUs === timestampUs) return cursorSamples[idx];
		const a = cursorSamples[idx - 1];
		const b = cursorSamples[idx];
		const range = b.timestampUs - a.timestampUs;
		const tLinear = range > 0 ? (timestampUs - a.timestampUs) / range : 0;
		// Apply the user's cursor-motion easing if set. The curve reshapes
		// the *interpolation parameter* between adjacent captured samples;
		// boolean states still flip at the midpoint of the linear param to
		// keep click/release timing predictable.
		const easing = store.cursorMotionEasing;
		const t = easing ? bezierY(easing, tLinear) : tLinear;
		return {
			timestampUs,
			x: a.x + (b.x - a.x) * t,
			y: a.y + (b.y - a.y) * t,
			visible: tLinear < 0.5 ? a.visible : b.visible,
			leftDown: tLinear < 0.5 ? a.leftDown : b.leftDown,
			rightDown: tLinear < 0.5 ? a.rightDown : b.rightDown,
		};
	}

	//  Color parsing 
	function hexToRgba(hex: string, alpha = 1): [number, number, number, number] {
		const s = hex.trim().replace(/^#/, "");
		if (s.length < 6) return [17 / 255, 17 / 255, 17 / 255, alpha];
		const r = parseInt(s.slice(0, 2), 16) / 255;
		const g = parseInt(s.slice(2, 4), 16) / 255;
		const b = parseInt(s.slice(4, 6), 16) / 255;
		return [r, g, b, alpha];
	}

	function parseGradient(value: string): [[number, number, number, number], [number, number, number, number]] {
		// Match the format used in store: linear-gradient(135deg, #f093fb 0%, #f5576c 100%)
		const matches = value.match(/#[0-9a-fA-F]{6,8}/g) ?? [];
		const a = matches[0] ? hexToRgba(matches[0]) : [0.94, 0.58, 0.98, 1] as [number, number, number, number];
		const b = matches[1] ? hexToRgba(matches[1]) : [0.96, 0.34, 0.42, 1] as [number, number, number, number];
		return [a, b];
	}

	//  Sizing 
	function resizeCanvas() {
		if (!canvasEl || !containerEl || !store.metadata) return false;
		const meta = store.metadata;
		if (!meta.width || !meta.height) return false;

		// Composition aspect = (video + 2*padding) on each side. Here padding is in
		// "video pixels"; we choose a render-buffer size that preserves the same
		// aspect and fits inside the container, capped to keep GPU cost reasonable.
		const padding = framePaddingPixels(store.padding, meta);
		const compW = meta.width + padding * 2;
		const compH = meta.height + padding * 2;

		const cw = containerEl.clientWidth;
		const ch = containerEl.clientHeight;
		if (cw <= 0 || ch <= 0) return false;

		// Fit composition into container preserving aspect
		const scale = Math.min(cw / compW, ch / compH);
		const cssW = Math.max(1, Math.floor(compW * scale));
		const cssH = Math.max(1, Math.floor(compH * scale));

		// Render at devicePixelRatio for crispness, capped at the composition's
		// native resolution (no point upscaling) and at 2160p to bound GPU cost.
		const dpr = Math.min(window.devicePixelRatio || 1, 2);
		const maxDim = 2160;
		let bufW = Math.min(Math.round(cssW * dpr), compW, maxDim);
		let bufH = Math.min(Math.round(cssH * dpr), compH, maxDim);
		// Maintain aspect after caps
		const bufScale = Math.min(bufW / compW, bufH / compH);
		bufW = Math.max(1, Math.floor(compW * bufScale));
		bufH = Math.max(1, Math.floor(compH * bufScale));

		canvasEl.style.width = `${cssW}px`;
		canvasEl.style.height = `${cssH}px`;
		if (canvasEl.width !== bufW || canvasEl.height !== bufH) {
			canvasEl.width = bufW;
			canvasEl.height = bufH;
		}
		return true;
	}

	//  Render 
	let loggedTexError = false;
	function uploadVideoFrame() {
		if (!gl || !videoTex || !videoEl) return false;
		if (videoEl.readyState < 2 /* HAVE_CURRENT_DATA */) return false;
		if (videoEl.videoWidth === 0 || videoEl.videoHeight === 0) return false;
		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, videoTex);
		// texImage2D from a video element is hardware-accelerated by the browser
		gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
		try {
			gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, videoEl);
		} catch (err) {
			if (!loggedTexError) {
				loggedTexError = true;
				console.error(
					`WebGL texImage2D failed for video (src=${videoEl.currentSrc || videoEl.src}):`,
					err,
				);
			}
			return false;
		}
		return true;
	}

	// Smoothly-eased zoom scale at `timeSec`. Returns 1.0 outside every
	// region; inside a region, ramps 1.0 → `region.scale` across `rampIn`
	// seconds shaped by `easeIn`, holds at `region.scale`, then ramps back
	// across `rampOut` shaped by `easeOut`. Matches the Rust
	// `ZoomRegion::scale_at` logic 1:1 so preview and export stay aligned.
	// Returns the current zoom state for `timeSec`:
	//  - scale: eased scale value (1.0 outside any region)
	//  - cx/cy: focus centre in video UV space (0.5/0.5 at rest), eased from
	//           the region's target centre in lockstep with the scale ramp
	//  - motionBlur: 0..1 strength multiplier of the active region (or 0)
	function evaluateZoomAt(timeSec: number): {
		scale: number;
		cx: number;
		cy: number;
		motionBlur: number;
	} {
		const regions = store.zoomRegions;
		for (const r of regions) {
			if (timeSec <= r.start || timeSec >= r.end) continue;
			const duration = Math.max(0, r.end - r.start);
			const half = duration * 0.5;
			const rampIn = Math.min(Math.max(0, r.rampIn), half);
			const rampOut = Math.min(Math.max(0, r.rampOut), half);
			const holdStart = r.start + rampIn;
			const holdEnd = r.end - rampOut;
			let phase: number;
			let curve;
			let atHold = false;
			if (timeSec < holdStart) {
				phase = rampIn > 0 ? (timeSec - r.start) / rampIn : 1;
				curve = r.easeIn;
			} else if (timeSec > holdEnd) {
				phase = rampOut > 0 ? (r.end - timeSec) / rampOut : 1;
				curve = r.easeOut;
			} else {
				atHold = true;
				phase = 1;
				curve = r.easeIn;
			}
			phase = Math.max(0, Math.min(1, phase));
			const eased = atHold ? 1 : bezierY(curve, phase);
			const scale = 1.0 + (r.scale - 1.0) * eased;
			const cx = 0.5 + ((r.centerX ?? 0.5) - 0.5) * eased;
			const cy = 0.5 + ((r.centerY ?? 0.5) - 0.5) * eased;
			return { scale, cx, cy, motionBlur: r.motionBlur ?? 0 };
		}
		return { scale: 1.0, cx: 0.5, cy: 0.5, motionBlur: 0 };
	}

	function draw() {
		if (!gl || !program || !canvasEl || !store.metadata) return;
		if (!resizeCanvas()) return;

		// Refresh the smoothed cursor path if any of its inputs changed since
		// the last frame. Signature-based guard keeps this effectively free
		// (one string compare) when nothing's changed.
		ensureSmoothingCurrent();

		// Prefer the video element's current time for per-frame cursor & zoom
		// interpolation — `store.currentTime` only updates ~4×/sec via the
		// `timeupdate` event, so using it here caused visible cursor lag during
		// playback. Fall back to the store value when the video isn't available
		// (shouldn't happen inside draw but keeps the types honest).
		const playbackTime = videoEl ? videoEl.currentTime : store.currentTime;

		// Make sure the latest video frame is in the texture before sampling
		if (!uploadVideoFrame()) return;

		// Make sure background texture is current (fire-and-forget if it changed)
		void loadBackgroundIfNeeded();

		gl.viewport(0, 0, canvasEl.width, canvasEl.height);
		gl.clearColor(0, 0, 0, 1);
		gl.clear(gl.COLOR_BUFFER_BIT);

		gl.useProgram(program);

		gl.uniform2f(uniforms.u_canvasSize, canvasEl.width, canvasEl.height);

		// Padding maps from "video pixels" to canvas pixels by the buffer scale
		const meta = store.metadata!;
		const sourcePaddingPx = framePaddingPixels(store.padding, meta);
		const compW = meta.width + sourcePaddingPx * 2;
		const paddingPx = (sourcePaddingPx / compW) * canvasEl.width;
		gl.uniform1f(uniforms.u_paddingPx, paddingPx);

		// Background
		const bgType = store.backgroundType;
		let bgBlurPx = 0;
		if (bgType === "color") {
			gl.uniform1i(uniforms.u_bgType, 0);
			gl.uniform4fv(uniforms.u_bgColor, hexToRgba(store.backgroundValue || "#111111"));
		} else if (bgType === "gradient") {
			gl.uniform1i(uniforms.u_bgType, 1);
			const [a, b] = parseGradient(store.backgroundValue || "");
			gl.uniform4fv(uniforms.u_gradStart, a);
			gl.uniform4fv(uniforms.u_gradEnd, b);
		} else {
			// wallpaper / image
			if (bgTexReady) {
				gl.uniform1i(uniforms.u_bgType, 2);
				gl.activeTexture(gl.TEXTURE1);
				gl.bindTexture(gl.TEXTURE_2D, bgTex);
				// Map the 0..100 blur slider to a pixel radius. 100 ≈ 24px is
				// strong enough to be obvious without being too expensive.
				bgBlurPx = Math.max(0, store.backgroundBlur * 0.24);
			} else {
				// Fallback to dark color until image is loaded
				gl.uniform1i(uniforms.u_bgType, 0);
				gl.uniform4fv(uniforms.u_bgColor, [0.067, 0.067, 0.067, 1]);
			}
		}
		gl.uniform1f(uniforms.u_bgBlurPx, bgBlurPx);

		// Border radius — user-provided as a percentage of the shorter video edge
		// (0..50). Convert to canvas pixels using the same scale as padding.
		const shorterEdge = Math.min(meta.width, meta.height);
		const radiusSource = ((store.borderRadius ?? 0) / 100) * shorterEdge;
		// Same video-pixel → canvas-pixel scale as the padding calculation.
		const radiusPx = (radiusSource / compW) * canvasEl.width;
		gl.uniform1f(uniforms.u_borderRadiusPx, Math.max(0, radiusPx));

		// Zoom — eased per-frame scale + focus centre + motion-blur strength.
		const zoom = evaluateZoomAt(playbackTime);
		gl.uniform2f(uniforms.u_zoomCenter, zoom.cx, zoom.cy);
		gl.uniform1f(uniforms.u_zoomScale, zoom.scale);

		// Motion blur: radius scales with |d(scale)/dt| so hold frames are
		// sharp and ramps smear toward the focus point. dt = 1/60 matches the
		// preview's baseline and is fine as a finite-difference step since the
		// ramp shapes are C1-continuous beziers.
		let motionBlurPx = 0;
		if (zoom.motionBlur > 0.001 && zoom.scale > 1.0001) {
			const dt = 1 / 60;
			const next = evaluateZoomAt(playbackTime + dt);
			const dScaleDt = Math.abs(next.scale - zoom.scale) / dt;
			// k = 30 px per unit-scale-per-second is a good default at 1080p;
			// cap at 20 px to keep the 7-tap sample cheap.
			motionBlurPx = Math.min(20, zoom.motionBlur * dScaleDt * 30);
		}
		gl.uniform1f(uniforms.u_motionBlurPx, motionBlurPx);

		// Cursor
		const cs = store.cursorSettings;
		let cursorAlpha = 0;
		let highlightAlpha = 0;
		let cursorPosX = 0;
		let cursorPosY = 0;
		let cursorPressed = false;
		if (cs.enabled && cursorSamples.length > 0) {
			const ts = Math.max(0, playbackTime) * 1_000_000;

			// Idle visibility — smooth fade rather than a binary cut. Outside
			// any idle period the alpha is 1; deep inside it's 0; near each
			// boundary we linearly ramp over CURSOR_IDLE_FADE_US so the cursor
			// dissolves in/out instead of popping.
			const idleA = cs.hideWhenIdle ? idleAlphaAt(ts, cs.idleTimeout) : 1;

			if (idleA > 0) {
				const pos = interpolateCursor(ts);
				if (pos && pos.visible) {
					cursorAlpha = idleA;
					cursorPosX = pos.x / meta.width;
					cursorPosY = pos.y / meta.height;
					cursorPressed = !!(pos.leftDown || pos.rightDown);
					if (cs.highlightClicks && cursorPressed) {
						highlightAlpha = (cs.highlightOpacity / 100) * idleA;
					}
				}
			}
		}
		// When the user picks a custom SVG cursor style, the WebGL shader's
		// dot path is suppressed and the HTML <img> overlay below paints the
		// cursor instead. The shader still renders the click-highlight halo.
		const usingSvgCursor = cs.enabled && cs.style !== "dot";
		const overlayVisible = usingSvgCursor && cursorAlpha > 0;
		gl.uniform2f(uniforms.u_cursorPos, cursorPosX, cursorPosY);
		gl.uniform1f(
			uniforms.u_cursorVisible,
			usingSvgCursor ? 0 : cursorAlpha,
		);
		// Push to reactive state so the HTML overlay updates each frame.
		// We mirror the shader's cursor-zoom math so the SVG tracks the dot
		// pixel-for-pixel — the shader applies `(uv - center)*scale + center`
		// to the cursor UV; we do the same here before mapping to canvas px.
		let svgUvX = cursorPosX;
		let svgUvY = cursorPosY;
		if (zoom.scale > 1.0001) {
			svgUvX = (cursorPosX - zoom.cx) * zoom.scale + zoom.cx;
			svgUvY = (cursorPosY - zoom.cy) * zoom.scale + zoom.cy;
		}
		const compH_local = meta.height + sourcePaddingPx * 2;
		// Sprite design size in source pixels; the same `* 2` factor the dot
		// uses for radius, doubled because the sprite is a full-bleed bbox
		// rather than a centered dot.
		const spriteSourcePx = cs.size * 16;
		svgCursor = {
			visible: overlayVisible,
			alpha: cursorAlpha,
			styleId: cs.style,
			pressed: cursorPressed,
			canvasX: sourcePaddingPx + svgUvX * meta.width,
			canvasY: sourcePaddingPx + svgUvY * meta.height,
			compW,
			compH: compH_local,
			spritePx: spriteSourcePx,
		};
		// Match Rust: cursor radius = size * 2 (in source pixels), scaled to canvas
		const cursorRadiusCanvas = (cs.size * 2 * canvasEl.width) / compW;
		gl.uniform1f(uniforms.u_cursorRadius, Math.max(2, cursorRadiusCanvas));
		gl.uniform4fv(uniforms.u_cursorColor, [1, 1, 1, 0.9]);
		const [hr, hg, hb] = hexToRgba(cs.highlightColor || "#3b82f6");
		gl.uniform4fv(uniforms.u_highlightColor, [hr, hg, hb, 1]);
		gl.uniform1f(uniforms.u_highlightAlpha, highlightAlpha);

		// Drop shadow — offsets/blur/spread expressed in "video pixels" so the
		// look scales consistently with the canvas at different container
		// sizes. Same source-pixel → canvas-pixel factor as padding/radius.
		const shadow = store.shadow;
		if (shadow.enabled && shadow.opacity > 0) {
			const vpToCanvas = canvasEl.width / compW;
			gl.uniform1i(uniforms.u_shadowEnabled, 1);
			gl.uniform1f(uniforms.u_shadowBlurPx, Math.max(0.5, shadow.blur * vpToCanvas));
			gl.uniform1f(uniforms.u_shadowSpreadPx, Math.max(0, shadow.spread * vpToCanvas));
			gl.uniform2f(uniforms.u_shadowOffsetPx, 0, shadow.offsetY * vpToCanvas);
			const [sr, sg, sb] = hexToRgba(shadow.color || "#000000");
			gl.uniform4fv(uniforms.u_shadowColor, [sr, sg, sb, shadow.opacity / 100]);
		} else {
			gl.uniform1i(uniforms.u_shadowEnabled, 0);
			gl.uniform4fv(uniforms.u_shadowColor, [0, 0, 0, 0]);
		}

		gl.drawArrays(gl.TRIANGLES, 0, 6);
	}

	function requestRedraw() {
		if (rafHandle !== null) return;
		rafHandle = requestAnimationFrame(() => {
			rafHandle = null;
			draw();
		});
	}

	//  Playback frame loop (rVFC) 
	type RVFCMetadata = { mediaTime: number; presentedFrames: number };
	type VideoElWithRVFC = HTMLVideoElement & {
		requestVideoFrameCallback?: (cb: (now: number, metadata: RVFCMetadata) => void) => number;
		cancelVideoFrameCallback?: (handle: number) => void;
	};

	function startVideoFrameLoop() {
		const v = videoEl as VideoElWithRVFC | null;
		if (!v || typeof v.requestVideoFrameCallback !== "function") {
			// Fallback: drive via RAF whenever the video advances
			return;
		}
		const tick = (_now: number, _meta: RVFCMetadata) => {
			draw();
			rvfcHandle = v.requestVideoFrameCallback!(tick);
		};
		rvfcHandle = v.requestVideoFrameCallback(tick);
	}

	function stopVideoFrameLoop() {
		if (rvfcHandle === null) return;
		const v = videoEl as VideoElWithRVFC | null;
		if (v && typeof v.cancelVideoFrameCallback === "function") {
			v.cancelVideoFrameCallback(rvfcHandle);
		}
		rvfcHandle = null;
	}

	//  Lifecycle & reactive wiring 
	onMount(() => {
		initGL();
		const ro = new ResizeObserver(() => requestRedraw());
		if (containerEl) ro.observe(containerEl);
		return () => ro.disconnect();
	});

	onDestroy(() => {
		stopVideoFrameLoop();
		if (rafHandle !== null) cancelAnimationFrame(rafHandle);
		if (gl) {
			if (videoTex) gl.deleteTexture(videoTex);
			if (bgTex) gl.deleteTexture(bgTex);
			if (program) gl.deleteProgram(program);
		}
	});

	// Cursor track (re)load when path changes
	$effect(() => {
		void cursorPath;
		void loadCursorTrackIfNeeded();
	});

	// Background (re)load when type/value changes, or when an asset:<id>
	// download lands and the cached path becomes available.
	$effect(() => {
		void store.backgroundType;
		void store.backgroundValue;
		if (store.backgroundValue.startsWith("asset:") && !store.backgroundValue.startsWith("asset://")) {
			const id = store.backgroundValue.slice("asset:".length);
			void assetsStore.paths[id];
			void assetsStore.thumbPaths[id];
		}
		void loadBackgroundIfNeeded();
		requestRedraw();
	});

	// Redraw on any visual property change
	$effect(() => {
		// Track every dependency that affects the rendered frame
		void store.padding;
		void store.backgroundBlur;
		void store.borderRadius;
		void store.currentTime;
		void store.metadata;
		void store.cursorSettings;
		void store.zoomRegions;
		void store.shadow;
		requestRedraw();
	});

	// Start/stop the per-video-frame draw loop with playback
	$effect(() => {
		if (store.isPlaying) {
			startVideoFrameLoop();
		} else {
			stopVideoFrameLoop();
			requestRedraw();
		}
	});

	// Hook video element events
	function handleSeeked() {
		requestRedraw();
		onSeeked?.();
	}
	function handleLoadedData() {
		isReady = true;
		requestRedraw();
		onReady();
	}
</script>

<div
	bind:this={containerEl}
	class="relative flex h-full w-full max-w-280 items-center justify-center overflow-hidden"
>
	<div bind:this={previewRectEl} class="relative inline-block">
		<canvas
			bind:this={canvasEl}
			class="block max-h-full max-w-full"
		></canvas>
		<AnnotationOverlay {store} {videoEl} targetEl={previewRectEl} />
		<TextAnnotationLayer {store} {videoEl} targetEl={previewRectEl} />
		<FocusOverlay {store} {videoEl} targetEl={previewRectEl} />
		{#if svgCursor.visible}
			{@const style = CURSOR_STYLES.find((s) => s.id === svgCursor.styleId)}
			{@const stateKey = svgCursor.pressed && style?.pressedSvg ? "press" : "rest"}
			{@const hot =
				stateKey === "press" && style?.pressedHotspot
					? style.pressedHotspot
					: (style?.hotspot ?? { x: 32, y: 32 })}
			<!-- Custom SVG cursor: positioned at the cursor sample's
			     source-pixel coordinates, mapped into the canvas's CSS rect.
			     The image swaps between rest and press sprites so styles like
			     macOS show the link-pointing hand while the captured cursor
			     is held down. Hotspot is offset per state so each sprite's
			     tip lands on the captured pointer position. -->
			<img
				src={cursorStyleDataUrl(svgCursor.styleId, stateKey)}
				alt=""
				draggable="false"
				class="pointer-events-none absolute"
				style="
					left: {(svgCursor.canvasX / svgCursor.compW) * 100}%;
					top: {(svgCursor.canvasY / svgCursor.compH) * 100}%;
					width: {(svgCursor.spritePx / svgCursor.compW) * 100}%;
					transform: translate(-{(hot.x / 64) * 100}%, -{(hot.y / 64) * 100}%);
					opacity: {svgCursor.alpha};
					filter: drop-shadow(0 1px 1.5px rgb(0 0 0 / 0.5));
				"
			/>
		{/if}
	</div>

	{#if videoSrc}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			bind:this={videoEl}
			src={videoSrc}
			crossorigin="anonymous"
			ontimeupdate={onTimeUpdate}
			onended={onEnded}
			onloadedmetadata={onLoadedMetadata}
			onloadeddata={handleLoadedData}
			oncanplay={handleLoadedData}
			onseeked={handleSeeked}
			onerror={onError}
			class="pointer-events-none absolute h-px w-px opacity-0"
			style="visibility: hidden;"
			playsinline
			preload="auto"
			muted
		></video>
	{/if}

	{#if !isReady}
		<div class="pointer-events-none absolute inset-0 flex items-center justify-center gap-2 text-sm text-muted-foreground">
			<Spinner class="size-4" />
			<span>Loading preview</span>
		</div>
	{/if}
</div>
