<script lang="ts">
// Live caption overlay over the preview. Reads the transcript + style from the
// store, maps the playhead back to source time, and paints the active caption
// onto a 2D canvas via the SAME resolve + paint path the export burn-in uses
// (caption-render) — so preview == export by construction, no DOM renderer to
// drift. Fills `previewRectEl` (the composited output rect), so the caption's
// video-relative placement matches the frame.
import { paintCaptionChunk, resolveCaptionView } from "$lib/captions/caption-render";
import { computeCanvasGeometry } from "$lib/canvas-geometry";
import { ensureFontLoaded } from "$lib/fonts/font-options";
import type { EditorStore } from "$lib/stores/editor-store.svelte";
import { outputToOriginal } from "$lib/timeline/time-map";

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

// Paint the active caption every time the playhead (OUTPUT time), style, or size
// changes. The playhead is OUTPUT time; the transcript is SOURCE time, so map
// back through the time map — captions and per-word timing stay synced across
// cuts and per-segment speed. Entrance is clocked on OUTPUT time (viewer-rate).
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
	const nowOrig = outputToOriginal(store.timeMap, store.currentTime);
	const view = resolveCaptionView(store.transcript, store.captionStyle, store.timeMap, nowOrig);
	if (!view) return;
	const g = computeCanvasGeometry(m.width, m.height, store.padding, store.outputAspect);
	paintCaptionChunk(ctx, view, store.captionStyle, store.currentTime, {
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
