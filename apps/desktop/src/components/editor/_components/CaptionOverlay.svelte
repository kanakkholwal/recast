<script lang="ts">
// Live caption overlay over the preview. This is the ADAPTER: it reads the
// transcript + style from the store, maps the playhead back to source time,
// places the caption relative to the video rect, and picks the chunk active at
// the playhead. The LOOK (pill, per-word colour, entrance) is rendered by the
// shared <CaptionBox> from @recast/captions, so the editor and the web player
// render captions identically. Sits inside `previewRectEl`, so `cqh` font
// sizing tracks the preview size.
import {
	activeChunkIndex,
	activeWordIndex,
	captionHeightFrac,
	captionTopFrac,
	chunkWords,
	isStaticAnimation,
	resolveCaptionAnimation,
	spokenWordCount,
} from '@recast/captions';
import CaptionBox from '@recast/captions/box';
import { activeClippedSegment, clipWordsToSpan } from '$lib/captions/clip-with-cuts';
import { computeCanvasGeometry } from '$lib/canvas-geometry';
import { ensureFontLoaded } from '$lib/fonts/font-options';
import type { EditorStore } from '$lib/stores/editor-store.svelte';
import { outputToOriginal, spanAtOriginal } from '$lib/timeline/time-map';

let { store }: { store: EditorStore } = $props();

// Fetch + register the selected Google font (idempotent) so the preview
// renders it. Covers picker changes and reloading a saved project.
$effect(() => {
	ensureFontLoaded(store.captionStyle.fontFamily, store.captionStyle.fontWeight);
});

// The playhead is OUTPUT time; the transcript is SOURCE time. Map back through
// the time map so captions (and per-word timing) stay synced across cuts and
// per-segment speed changes.
const nowOrig = $derived(outputToOriginal(store.timeMap, store.currentTime));

// The kept source-time span that contains the playhead. Null when the
// playhead is inside a cut (the gap between two kept spans) — in that
// case no caption should be on screen. Computed once per `nowOrig` change
// and reused by both the segment lookup and the word clipping below.
const keptSpan = $derived(spanAtOriginal(store.timeMap, nowOrig));

// Active segment intersected with the kept span. We use the CLIPPED
// segment (not the original) so a caption that spans a cut only displays
// the kept portion — without this, a caption that crosses a cut stays
// on screen for the full source-time duration, which appears to
// "outlast" the cut and drift relative to the audio.
const active = $derived.by(() => {
	const t = store.transcript;
	if (!t || !store.captionStyle.enabled) return null;
	const span = keptSpan;
	if (!span) return null;
	const clipped = activeClippedSegment(t.segments, span, nowOrig);
	if (!clipped) return null;
	// Replace the segment's words with the span-clipped versions so the
	// per-word animation re-times correctly across the cut. The chunk
	// boundary math re-runs against the clipped words.
	return {
		...clipped.segment,
		start: clipped.visible.start,
		end: clipped.visible.end,
		words: clipWordsToSpan(clipped.segment.words, span),
	};
});

const anim = $derived(resolveCaptionAnimation(store.captionStyle.animation));
const animated = $derived(!!active && active.words.length > 0 && !isStaticAnimation(anim));

// The chunk to show plus its progress. For a static line the whole segment is
// one chunk with every word "spoken" (so nothing renders muted). `key`
// re-mounts <CaptionBox> when the chunk changes, replaying the entrance.
const _view = $derived.by(() => {
	if (!active) return null;
	if (active.words.length === 0) {
		// Defensive: a segment with text but no per-word timing renders as one word.
		const w = [{ start: active.start, end: active.end, text: active.text }];
		return { key: active.id, words: w, spoken: 1, wi: -1 };
	}
	if (!animated) {
		return { key: active.id, words: active.words, spoken: active.words.length, wi: -1 };
	}
	const runs = chunkWords(active.words, anim);
	const ci = activeChunkIndex(runs, nowOrig);
	const chunk = runs[ci];
	if (!chunk) return null;
	return {
		// Include the visible window in the key so a cut-cross inside a
		// segment re-mounts the box (a new visual identity, not a continuation).
		key: `${active.id}:${ci}:${active.start}:${active.end}`,
		words: chunk.words,
		spoken: spokenWordCount(chunk.words, nowOrig),
		wi: activeWordIndex(chunk.words, nowOrig, anim.holdGaps),
	};
});

// The video rect inside the output canvas (with padding + aspect bars around
// it). Captions are placed relative to it so top/bottom sit in the padding,
// not over the video, mirroring the Rust ASS generator.
const _box = $derived.by(() => {
	const s = store.captionStyle;
	const m = store.metadata;
	const g =
		m?.width && m.height
			? computeCanvasGeometry(m.width, m.height, store.padding, store.outputAspect)
			: null;
	const vLeft = g ? g.videoX / g.canvasW : 0;
	const vRight = g ? (g.videoX + g.videoW) / g.canvasW : 1;
	const vTop = g ? g.videoY / g.canvasH : 0;
	const vBottom = g ? (g.videoY + g.videoH) / g.canvasH : 1;
	const cap = captionHeightFrac(s.fontSizePct, s.maxLines);
	const topFrac = captionTopFrac(s.position, s.offsetPct, cap, { top: vTop, bottom: vBottom });
	// `topFrac === null` -> centre vertically on the video.
	const vertical =
		topFrac === null
			? `top: ${((vTop + vBottom) / 2) * 100}%; transform: translateY(-50%);`
			: `top: ${topFrac * 100}%;`;
	return { leftPct: vLeft * 100, widthPct: (vRight - vLeft) * 100, vertical };
});
</script>

{#if active && _view}
	{@const s = store.captionStyle}
	<div class="caption-layer pointer-events-none absolute inset-0">
		<div
			class="caption-box absolute flex px-[4%]"
			class:justify-start={s.align === 'left'}
			class:justify-center={s.align === 'center'}
			class:justify-end={s.align === 'right'}
			style="left: {_box.leftPct}%; width: {_box.widthPct}%; {_box.vertical}"
		>
			{#key _view.key}
				<CaptionBox
					words={_view.words}
					style={s}
					{anim}
					spokenCount={_view.spoken}
					activeIndex={_view.wi}
					fontSize="{s.fontSizePct}cqh"
				/>
			{/key}
		</div>
	</div>
{/if}

<style>
	/* Establish a size container so the caption's `cqh` font scales with the
	   preview rectangle (which this layer fills). */
	.caption-layer {
		container-type: size;
	}
</style>