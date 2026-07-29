<script lang="ts">
// Live caption overlay over the preview. Reads the transcript + style from the
// store, maps the playhead back to source time, and paints the active caption
// onto a 2D canvas via the SAME resolve + paint path the export burn-in uses
// (caption-render) — so preview == export by construction, no DOM renderer to
// drift. Fills `previewRectEl` (the composited output rect), so the caption's
// video-relative placement matches the frame.
import { captionClocks, paintCaptionChunk, resolveCaptionView } from "$lib/captions/caption-render";
import { computeCanvasGeometry } from "$lib/canvas-geometry";
import { ensureFontLoaded } from "$lib/fonts/font-options";
import type { EditorStore } from "$lib/stores/editor-store.svelte";

let { store }: { store: EditorStore } = $props();

let canvasEl = $state<HTMLCanvasElement | null>(null);
// CSS box size, tracked reactively so the backing store follows the preview.
let cssW = $state(0);
let cssH = $state(0);

// Fetch + register the selected Google font (idempotent) so the canvas paints
// it. Covers picker changes and reloading a saved project; the rAF-driven
// redraw below picks up the real face once it loads.
$effect(() => {
	ensureFontLoaded(store.captionStyle.fontFamily, store.captionStyle.fontWeight);
});

// Paint the active caption whenever the playhead, style, or size changes.
// `store.currentTime` is SOURCE time, which resolves the chunk directly; the
// entrance is clocked on OUTPUT time (viewer-rate) via the time map, so per-word
// highlight and entrance stay correct across trims, cuts, and per-segment speed.
$effect(() => {
	const canvas = canvasEl;
	if (!canvas) return;
	const dpr = window.devicePixelRatio || 1;
	const cw = Math.max(1, Math.round(cssW * dpr));
	const ch = Math.max(1, Math.round(cssH * dpr));
	if (canvas.width !== cw) canvas.width = cw;
	if (canvas.height !== ch) canvas.height = ch;
	const ctx = canvas.getContext("2d");
	if (!ctx) return;
	ctx.clearRect(0, 0, cw, ch);

	if (!store.captionStyle.enabled) return;
	const m = store.metadata;
	if (!m?.width || !m?.height) return;
	const { sourceSec, outputSec } = captionClocks(store.timeMap, store.currentTime);
	// captionTranscript is rescaled onto the video/timeMap axis (fixes audio-vs-
	// video CFR drift); every caption surface reads it so they stay in sync.
	const view = resolveCaptionView(
		store.captionTranscript,
		store.captionStyle,
		store.timeMap,
		sourceSec,
	);
	if (!view) return;
	const g = computeCanvasGeometry(m.width, m.height, store.padding, store.outputAspect);
	paintCaptionChunk(ctx, view, store.captionStyle, outputSec, {
		videoLeftFrac: g.videoX / g.canvasW,
		videoRightFrac: (g.videoX + g.videoW) / g.canvasW,
		videoTopFrac: g.videoY / g.canvasH,
		videoBottomFrac: (g.videoY + g.videoH) / g.canvasH,
		canvasPxW: cw,
		canvasPxH: ch,
	});
});
</script>

<canvas
	bind:this={canvasEl}
	bind:clientWidth={cssW}
	bind:clientHeight={cssH}
	class="pointer-events-none absolute inset-0 h-full w-full"
></canvas>
