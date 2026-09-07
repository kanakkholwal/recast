<script lang="ts">
// Paints via the same resolve and paint path the export burn-in uses, so preview equals export by construction.

import { computeCanvasGeometry } from "../../lib/canvas-geometry";
import {
	captionClocks,
	paintCaptionChunk,
	resolveCaptionView,
} from "../../lib/captions/caption-render";
import { ensureFontLoaded } from "../../lib/fonts/font-options";
import type { EditorStore } from "../../stores/editor-store.svelte";

// Rides the rAF-smooth picture clock, not the 25Hz store time: a sub-second entrance falls between throttled samples.
let { store, previewTime }: { store: EditorStore; previewTime?: number } = $props();

let canvasEl = $state<HTMLCanvasElement | null>(null);
// CSS box size, tracked reactively so the backing store follows the preview.
let cssW = $state(0);
let cssH = $state(0);
// Local entrance clock for the PAUSED replay below; ignored during playback, where the picture clock drives it.
let replaySec = $state(0);

// Fetch + register the selected Google font (idempotent) so the canvas paints it.
$effect(() => {
	ensureFontLoaded(store.captionStyle.fontFamily, store.captionStyle.fontWeight);
});

const clockSec = $derived(previewTime ?? store.currentTime);

// `captionTranscript` is pre-rescaled onto the video axis, so resolve directly against SOURCE time.
const view = $derived.by(() => {
	if (!store.captionStyle.enabled) return null;
	const m = store.metadata;
	if (!m?.width || !m?.height) return null;
	const { sourceSec } = captionClocks(store.timeMap, clockSec);
	return resolveCaptionView(store.captionTranscript, store.captionStyle, store.timeMap, sourceSec);
});

// Paused, the playhead is frozen past the entrance, so ramp a local clock through it to give the Motion tab a live preview.
$effect(() => {
	const v = view;
	if (store.isPlaying || !v || v.anim.entrance === "none") return;
	const entranceSec = Math.max(0, v.anim.entranceMs) / 1000;
	if (!(entranceSec > 0)) return;
	replaySec = 0;
	let start: number | null = null;
	let handle = requestAnimationFrame(function tick(now) {
		if (start === null) start = now;
		replaySec = Math.min(entranceSec, (now - start) / 1000);
		if (replaySec < entranceSec) handle = requestAnimationFrame(tick);
	});
	return () => cancelAnimationFrame(handle);
});

// Paint on every clock tick, style/size change, or replay frame.
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

	const v = view;
	const m = store.metadata;
	if (!v || !m?.width || !m?.height) return;
	const { outputSec } = captionClocks(store.timeMap, clockSec);
	// Playing, the picture clock drives the entrance; paused, it replays from the chunk's own start.
	const entranceClock = store.isPlaying ? outputSec : v.chunkStartOutput + replaySec;
	const g = computeCanvasGeometry(m.width, m.height, store.padding, store.outputAspect);
	paintCaptionChunk(ctx, v, store.captionStyle, entranceClock, {
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
