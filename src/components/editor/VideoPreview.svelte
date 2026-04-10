<script lang="ts">
	import type { EditorStore } from "$lib/stores/editor-store.svelte";
	import { Spinner } from "$components/ui/spinner";
	import { convertFileSrc } from "@tauri-apps/api/core";
	import { onMount, onDestroy } from "svelte";

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
	}: Props = $props();

	// ── DOM refs & GL state ──────────────────────────────────────────────
	let canvasEl: HTMLCanvasElement | null = $state(null);
	let containerEl: HTMLDivElement | null = $state(null);
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
	let cursorSamples: CursorSampleJS[] = [];
	let idlePeriods: IdlePeriodJS[] = [];
	let loadedCursorPath = "";

	// ── Shaders ──────────────────────────────────────────────────────────
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
uniform vec2 u_zoomCenter;        // [0..1] in video UV
uniform float u_zoomScale;        // 1.0 = no zoom

uniform vec2 u_cursorPos;         // [0..1] in video UV
uniform float u_cursorVisible;    // 0 or 1
uniform float u_cursorRadius;     // pixels (canvas)
uniform vec4 u_cursorColor;
uniform vec4 u_highlightColor;
uniform float u_highlightAlpha;   // 0 if no click highlight

in vec2 v_uv;
out vec4 frag;

vec4 sampleBackground(vec2 uv) {
	if (u_bgType == 0) return u_bgColor;
	if (u_bgType == 1) {
		// Diagonal gradient (matches the 135deg gradient presets in the store)
		float t = clamp((uv.x + uv.y) * 0.5, 0.0, 1.0);
		return mix(u_gradStart, u_gradEnd, t);
	}
	return texture(u_background, uv);
}

void main() {
	vec2 canvasPx = v_uv * u_canvasSize;

	vec2 videoMin = vec2(u_paddingPx);
	vec2 videoMax = u_canvasSize - vec2(u_paddingPx);
	vec2 videoSize = max(videoMax - videoMin, vec2(1.0));

	bool inVideo = all(greaterThanEqual(canvasPx, videoMin)) && all(lessThan(canvasPx, videoMax));

	vec4 color = sampleBackground(v_uv);

	if (inVideo) {
		// uv inside the displayed video rect, [0..1]
		vec2 videoUV = (canvasPx - videoMin) / videoSize;

		// Apply zoom: shrink uv toward zoom center
		if (u_zoomScale > 1.0001) {
			videoUV = (videoUV - u_zoomCenter) / u_zoomScale + u_zoomCenter;
			videoUV = clamp(videoUV, 0.0, 1.0);
		}

		color = texture(u_video, videoUV);

		// Cursor overlay (drawn on top of video, clipped to video region)
		if (u_cursorVisible > 0.5) {
			// Map cursor from video UV → displayed pixel position, with zoom transform
			vec2 cursorUV = u_cursorPos;
			if (u_zoomScale > 1.0001) {
				cursorUV = (cursorUV - u_zoomCenter) * u_zoomScale + u_zoomCenter;
			}

			// Only render cursor if it's still in the visible region after zoom
			if (cursorUV.x >= 0.0 && cursorUV.x <= 1.0 && cursorUV.y >= 0.0 && cursorUV.y <= 1.0) {
				vec2 cursorPx = videoMin + cursorUV * videoSize;
				float dist = length(canvasPx - cursorPx);

				// Click highlight (drawn first, beneath cursor dot)
				if (u_highlightAlpha > 0.0) {
					float hr = u_cursorRadius * 6.0;
					float ha = (1.0 - smoothstep(hr - 4.0, hr, dist)) * u_highlightAlpha;
					color = mix(color, u_highlightColor, ha);
				}

				// Cursor dot with antialiased edge
				float cd = 1.0 - smoothstep(u_cursorRadius - 1.5, u_cursorRadius, dist);
				color = mix(color, u_cursorColor, cd * u_cursorColor.a);
			}
		}
	}

	frag = vec4(color.rgb, 1.0);
}`;

	// ── GL helpers ───────────────────────────────────────────────────────
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
			"u_zoomCenter",
			"u_zoomScale",
			"u_cursorPos",
			"u_cursorVisible",
			"u_cursorRadius",
			"u_cursorColor",
			"u_highlightColor",
			"u_highlightAlpha",
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

	// ── Background loading ──────────────────────────────────────────────
	async function loadBackgroundIfNeeded() {
		if (!gl || !bgTex) return;
		const type = store.backgroundType;
		const value = store.backgroundValue;
		const key = `${type}|${value}`;
		if (key === lastBgKey) return;
		lastBgKey = key;

		if (type !== "wallpaper" && type !== "image") {
			bgTexReady = false;
			return;
		}

		try {
			const img = new Image();
			img.crossOrigin = "anonymous";
			// Wallpapers are at /wallpapers/wallpaperN.png (served from static/)
			img.src = value;
			await img.decode();
			if (lastBgKey !== key) return; // Superseded by another load
			gl.bindTexture(gl.TEXTURE_2D, bgTex);
			gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
			bgTexReady = true;
		} catch (err) {
			console.warn("Background image load failed:", err);
			bgTexReady = false;
		}
	}

	// ── Cursor track loading ────────────────────────────────────────────
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
			cursorSamples = json.samples ?? [];
			idlePeriods = json.idlePeriods ?? [];
			loadedCursorPath = cursorPath;
		} catch (err) {
			console.warn("Cursor track load failed:", err);
			cursorSamples = [];
			idlePeriods = [];
		}
	}

	// ── Cursor interpolation (mirror of cursor::smoothing::interpolate_at) ─
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
		const t = range > 0 ? (timestampUs - a.timestampUs) / range : 0;
		return {
			timestampUs,
			x: a.x + (b.x - a.x) * t,
			y: a.y + (b.y - a.y) * t,
			visible: t < 0.5 ? a.visible : b.visible,
			leftDown: t < 0.5 ? a.leftDown : b.leftDown,
			rightDown: t < 0.5 ? a.rightDown : b.rightDown,
		};
	}

	// ── Color parsing ────────────────────────────────────────────────────
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

	// ── Sizing ───────────────────────────────────────────────────────────
	function resizeCanvas() {
		if (!canvasEl || !containerEl || !store.metadata) return false;
		const meta = store.metadata;
		if (!meta.width || !meta.height) return false;

		// Composition aspect = (video + 2*padding) on each side. Here padding is in
		// "video pixels"; we choose a render-buffer size that preserves the same
		// aspect and fits inside the container, capped to keep GPU cost reasonable.
		const padding = Math.max(0, store.padding);
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

	// ── Render ───────────────────────────────────────────────────────────
	function uploadVideoFrame() {
		if (!gl || !videoTex || !videoEl) return false;
		if (videoEl.readyState < 2 /* HAVE_CURRENT_DATA */) return false;
		if (videoEl.videoWidth === 0 || videoEl.videoHeight === 0) return false;
		gl.activeTexture(gl.TEXTURE0);
		gl.bindTexture(gl.TEXTURE_2D, videoTex);
		// texImage2D from a video element is hardware-accelerated by the browser
		gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, videoEl);
		return true;
	}

	function findActiveZoom(timeSec: number) {
		const regions = store.zoomRegions;
		for (const r of regions) {
			if (timeSec >= r.start && timeSec <= r.end) return r;
		}
		return null;
	}

	function draw() {
		if (!gl || !program || !canvasEl || !store.metadata) return;
		if (!resizeCanvas()) return;

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
		const compW = meta.width + store.padding * 2;
		const paddingPx = (store.padding / compW) * canvasEl.width;
		gl.uniform1f(uniforms.u_paddingPx, paddingPx);

		// Background
		const bgType = store.backgroundType;
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
			} else {
				// Fallback to dark color until image is loaded
				gl.uniform1i(uniforms.u_bgType, 0);
				gl.uniform4fv(uniforms.u_bgColor, [0.067, 0.067, 0.067, 1]);
			}
		}

		// Zoom
		const activeZoom = findActiveZoom(store.currentTime);
		if (activeZoom && activeZoom.scale > 1.0) {
			gl.uniform2f(uniforms.u_zoomCenter, 0.5, 0.5); // center crop, matching Rust behavior
			gl.uniform1f(uniforms.u_zoomScale, activeZoom.scale);
		} else {
			gl.uniform2f(uniforms.u_zoomCenter, 0.5, 0.5);
			gl.uniform1f(uniforms.u_zoomScale, 1.0);
		}

		// Cursor
		const cs = store.cursorSettings;
		let cursorVisible = 0;
		let highlightAlpha = 0;
		let cursorPosX = 0;
		let cursorPosY = 0;
		if (cs.enabled && cursorSamples.length > 0) {
			const ts = Math.max(0, store.currentTime) * 1_000_000;

			// Idle hide check
			let isIdle = false;
			if (cs.hideWhenIdle) {
				const thresholdUs = cs.idleTimeout * 1_000_000;
				for (const period of idlePeriods) {
					if (ts >= period.startUs + thresholdUs && ts <= period.endUs) {
						isIdle = true;
						break;
					}
				}
			}

			if (!isIdle) {
				const pos = interpolateCursor(ts);
				if (pos && pos.visible) {
					cursorVisible = 1;
					cursorPosX = pos.x / meta.width;
					cursorPosY = pos.y / meta.height;
					if (cs.highlightClicks && (pos.leftDown || pos.rightDown)) {
						highlightAlpha = cs.highlightOpacity / 100;
					}
				}
			}
		}
		gl.uniform2f(uniforms.u_cursorPos, cursorPosX, cursorPosY);
		gl.uniform1f(uniforms.u_cursorVisible, cursorVisible);
		// Match Rust: cursor radius = size * 2 (in source pixels), scaled to canvas
		const cursorRadiusCanvas = (cs.size * 2 * canvasEl.width) / compW;
		gl.uniform1f(uniforms.u_cursorRadius, Math.max(2, cursorRadiusCanvas));
		gl.uniform4fv(uniforms.u_cursorColor, [1, 1, 1, 0.9]);
		const [hr, hg, hb] = hexToRgba(cs.highlightColor || "#3b82f6");
		gl.uniform4fv(uniforms.u_highlightColor, [hr, hg, hb, 1]);
		gl.uniform1f(uniforms.u_highlightAlpha, highlightAlpha);

		gl.drawArrays(gl.TRIANGLES, 0, 6);
	}

	function requestRedraw() {
		if (rafHandle !== null) return;
		rafHandle = requestAnimationFrame(() => {
			rafHandle = null;
			draw();
		});
	}

	// ── Playback frame loop (rVFC) ───────────────────────────────────────
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

	// ── Lifecycle & reactive wiring ──────────────────────────────────────
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

	// Background (re)load when type/value changes
	$effect(() => {
		void store.backgroundType;
		void store.backgroundValue;
		void loadBackgroundIfNeeded();
		requestRedraw();
	});

	// Redraw on any visual property change
	$effect(() => {
		// Track every dependency that affects the rendered frame
		void store.padding;
		void store.currentTime;
		void store.metadata;
		void store.cursorSettings;
		void store.zoomRegions;
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
	}
	function handleLoadedData() {
		isReady = true;
		requestRedraw();
		onReady();
	}
</script>

<div
	bind:this={containerEl}
	class="relative flex h-full w-full max-w-280 items-center justify-center overflow-hidden rounded-2xl border border-border bg-muted/30"
>
	<canvas
		bind:this={canvasEl}
		class="block max-h-full max-w-full rounded-lg"
	></canvas>

	{#if videoSrc}
		<!-- svelte-ignore a11y_media_has_caption -->
		<video
			bind:this={videoEl}
			src={videoSrc}
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
