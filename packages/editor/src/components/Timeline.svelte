<script lang="ts">
import { AudioLines, Mic, Music2, Pencil, Scissors, Video, ZoomIn } from "@recast/icons";
import { onMount, untrack } from "svelte";
import { type AudioClip, clipEndSec } from "../lib/audio/music";
import type { TileProvider } from "../lib/timeline/filmstrip-source";
import { storyboardCrop } from "../lib/timeline/storyboard";
import { originalToOutput, outputToOriginal } from "../lib/timeline/time-map";
import type { EditorStore, ZoomRegion } from "../stores/editor-store.svelte";
import TimelineAnnotationLane from "./_components/timeline/TimelineAnnotationLane.svelte";
import TimelineAudioLane from "./_components/timeline/TimelineAudioLane.svelte";
import TimelineClipBar from "./_components/timeline/TimelineClipBar.svelte";
import TimelineCutLane from "./_components/timeline/TimelineCutLane.svelte";
import TimelineMusicLane from "./_components/timeline/TimelineMusicLane.svelte";
import TimelinePlayhead from "./_components/timeline/TimelinePlayhead.svelte";
import TimelineRuler from "./_components/timeline/TimelineRuler.svelte";
import TimelineToolbar from "./_components/timeline/TimelineToolbar.svelte";
import TimelineZoomLane from "./_components/timeline/TimelineZoomLane.svelte";
import { provideLaneDrag } from "./_components/timeline/timeline-drag.svelte";
import {
	clampTimelineZoom,
	effectiveFps as effFps,
	formatTimeByMode,
	frameStep as frameStepOf,
	greatestCommonDivisor,
	MIN_TIMELINE_ZOOM,
	minClipDuration as minClipDurOf,
	quantizeToFrame as quantizeToFrameOf,
	steppedZoom,
} from "./_components/timeline/timeline-helpers";
import { buildSnapTargets, snapTime } from "./_components/timeline/timeline-snap";
import {
	AUDIO_LANE_HEIGHT_PX,
	CLIP_LANE_HEIGHT_PX,
	CLIP_ROW_HEIGHT_PX,
	CUT_LANE_HEIGHT_PX,
	cardLayout,
	ZOOM_ROW_HEIGHT_PX,
} from "./_components/timeline/timeline-stack";
import { wheelIntent } from "./_components/timeline/timeline-wheel.logic";

// Orchestrator: scroll container, sizing, transport, keyboard routing and click-to-seek. Subviews live in `_components/timeline/`.

interface Props {
	store: EditorStore;
	videoEl?: HTMLVideoElement | null;
	tileProvider?: TileProvider | null;
	filmstripVersion?: number;
	/** Block structural edits (an agent owns the write lock) while leaving the
	 *  transport live, so the user can still scrub to watch what it changes. */
	readOnly?: boolean;
}

let {
	store,
	videoEl = null,
	tileProvider = null,
	filmstripVersion = 0,
	readOnly = false,
}: Props = $props();

let timelineEl: HTMLDivElement | undefined = $state();
let railInnerEl: HTMLDivElement | undefined = $state();
let isDraggingPlayhead = $state(false);
let timelineWidth = $state(900);
// Horizontal scroll offset, tracked so the clip bar can virtualize its tiles.
let scrollLeft = $state(0);
// Lane content shares the scroller's x-origin, so the clip bar's viewport math needs no offset.
const LANE_PAD = 0;

const SPEEDS = [0.25, 0.5, 1.0, 1.5, 2.0] as const;
let playbackSpeed = $state(1.0);

// Fixed lane heights come from timeline-stack; stacking lanes report theirs via `cardLayout`.
const LANE_GAP = 6;

// In the store, not here: the transport readout reads it too, so one setting flips every timecode.
const timeMode = $derived(store.timeMode);

// Persisted lane visibility; zoom and markup default to AUTO (null) because four always-on lanes squeezed the preview, and a user toggle stores a boolean that stops following content.
const VIEW_KEY = "recast.timeline.view";
/** `null` = follow the lane's content. */
type LanePref = boolean | null;
function loadView(): {
	waveform: boolean;
	zoom: LanePref;
	markup: LanePref;
	cuts: boolean;
	gaps: boolean;
} {
	if (typeof localStorage !== "undefined") {
		try {
			const raw = localStorage.getItem(VIEW_KEY);
			if (raw) {
				const v = JSON.parse(raw);
				return {
					// Migrate the old `clipContent` radio: anyone who chose the waveform still wants it, now as an overlay.
					waveform: typeof v.waveform === "boolean" ? v.waveform : v.clipContent === "waveform",
					// Anyone with a stored boolean chose it before auto existed; keep it.
					zoom: typeof v.zoom === "boolean" ? v.zoom : null,
					markup: typeof v.markup === "boolean" ? v.markup : null,
					cuts: v.cuts ?? v.silence ?? true,
					gaps: v.gaps === true,
				};
			}
		} catch {
			/* fall through to defaults */
		}
	}
	return { waveform: true, zoom: null, markup: null, cuts: true, gaps: false };
}
const _view = loadView();
let showAudioLane = $state(_view.waveform);
let showCutLane = $state(_view.cuts);
let zoomLanePref = $state<LanePref>(_view.zoom);
let markupLanePref = $state<LanePref>(_view.markup);
const showZoomLane = $derived(zoomLanePref ?? store.zoomRegions.length > 0);
const showMarkupLane = $derived(markupLanePref ?? store.annotations.length > 0);
// Lives in the store since it reshapes the render axis every lane reads; seeded from the persisted pref.
onMount(() => {
	store.showCutGaps = _view.gaps;
});
$effect(() => {
	if (typeof localStorage === "undefined") return;
	try {
		localStorage.setItem(
			VIEW_KEY,
			JSON.stringify({
				waveform: showAudioLane,
				// The PREF, not the resolved value: persisting the boolean would freeze a lane the moment content appeared.
				zoom: zoomLanePref,
				markup: markupLanePref,
				cuts: showCutLane,
				gaps: store.showCutGaps,
			}),
		);
	} catch {
		/* storage full / unavailable; view prefs are best-effort */
	}
});

// JKL transport: L/J cycle 1x, 2x, 4x and K parks. J reverses via rAF, since negative playbackRate is unreliable.
let shuttleDirection = $state<-1 | 0 | 1>(0);
let shuttleSpeedIndex = $state(0);
const SHUTTLE_SPEEDS = [1, 2, 4];
let reverseFrame = 0;

$effect(() => {
	if (!videoEl) return;
	// Legacy <video> path: the element is the clock, so per-segment speed must ride on its playbackRate.
	const segSpeed = store.segmentSpeedAtTime(store.currentTime);
	const transport =
		shuttleDirection === 1 ? SHUTTLE_SPEEDS[shuttleSpeedIndex] * playbackSpeed : playbackSpeed;
	videoEl.playbackRate = transport * segSpeed;
});

// Reverse-play loop. Held active only while shuttleDirection === -1.
function pumpReverse() {
	if (shuttleDirection !== -1 || !videoEl) {
		reverseFrame = 0;
		return;
	}
	const f = effectiveFps();
	const step = (SHUTTLE_SPEEDS[shuttleSpeedIndex] / f) * playbackSpeed;
	const next = Math.max(store.inPoint, store.currentTime - step);
	store.currentTime = next;
	videoEl.currentTime = next;
	if (next <= store.inPoint) {
		shuttleDirection = 0;
		shuttleSpeedIndex = 0;
		reverseFrame = 0;
		return;
	}
	reverseFrame = requestAnimationFrame(pumpReverse);
}

$effect(() => {
	if (shuttleDirection === -1 && reverseFrame === 0) {
		reverseFrame = requestAnimationFrame(pumpReverse);
	} else if (shuttleDirection !== -1 && reverseFrame !== 0) {
		cancelAnimationFrame(reverseFrame);
		reverseFrame = 0;
	}
});

// Only acts once the playhead crosses the margin, so manual scrolling mid-play is left alone until it runs off-screen.
$effect(() => {
	if (!store.isPlaying || isDraggingPlayhead || !timelineEl) return;
	const px = xOf(store.currentTime);
	// Cached and untracked: reading clientWidth then writing scrollLeft forces a full ruler layout, and the write re-enters this effect.
	untrack(() => {
		const view = timelineWidth;
		const left = scrollLeft;
		const margin = Math.min(view * 0.12, 120);
		if (px < left + margin || px > left + view - margin) {
			timelineEl!.scrollLeft = Math.max(0, px - margin);
		}
	});
});

// Round to the nearest frame so preview and export agree on the first and last kept frame.
function effectiveFps(): number {
	return effFps(store.metadata?.fps);
}
function quantizeToFrame(time: number): number {
	return quantizeToFrameOf(time, effectiveFps());
}
function frameStep(): number {
	return frameStepOf(effectiveFps());
}
function minClipDuration(): number {
	return minClipDurOf(effectiveFps());
}

function zoomTimeline(dir: number) {
	store.timelineZoom = steppedZoom(store.timelineZoom, dir, outputDuration, timelineWidth);
}

// timelineZoom=1 means "duration spans timelineWidth", so fit is just 1.0.
function zoomToFit() {
	store.timelineZoom = MIN_TIMELINE_ZOOM;
	requestAnimationFrame(() => {
		if (timelineEl) timelineEl.scrollLeft = 0;
	});
}

// Drives Zoom-to-selection for any timed selection; a clip selection is the spine, so it returns null.
function selectionSpan(): { start: number; end: number } | null {
	const sel = store.selection;
	if (!sel) return null;
	if (sel.kind === "zoom") {
		const r = store.zoomRegions.find((z) => z.id === sel.id);
		return r ? { start: r.start, end: r.end } : null;
	}
	if (sel.kind === "annotation") {
		const a = store.annotations.find((ann) => ann.id === sel.id);
		return a ? { start: a.start, end: a.end } : null;
	}
	if (sel.kind === "cut") {
		const c = store.cuts.find((cut) => cut.id === sel.id);
		return c ? { start: c.start, end: c.end } : null;
	}
	return null;
}
const hasFramableSelection = $derived(selectionSpan() !== null);

// Selection fills ~70% of the viewport (0.7 leaves context on both sides).
function zoomToSelection() {
	if (!timelineEl || duration <= 0) return;
	const span = selectionSpan();
	if (!span) return;
	const width = Math.max(0.001, span.end - span.start);
	const target = (duration / width) * 0.7;
	const nextZoom = clampTimelineZoom(target, outputDuration, timelineWidth);
	store.timelineZoom = nextZoom;
	requestAnimationFrame(() => {
		if (!timelineEl || outputDuration <= 0) return;
		const nextPps = (timelineEl.clientWidth * nextZoom) / outputDuration;
		// Center on the selection's midpoint in OUTPUT pixels.
		const center = (span.start + span.end) * 0.5;
		timelineEl.scrollLeft = Math.max(
			0,
			originalToOutput(store.renderMap, center) * nextPps - timelineEl.clientWidth * 0.5,
		);
	});
}

// Trim drags map this through a map FROZEN at drag-start, so the collapsed clip's left edge isn't a degenerate input.
function clientXToOutput(clientX: number): number {
	if (!timelineEl || pixelsPerSecond <= 0) return 0;
	const rect = timelineEl.getBoundingClientRect();
	return Math.max(0, (clientX - rect.left + timelineEl.scrollLeft) / pixelsPerSecond);
}

// For the global Alt+[ / Alt+] shortcuts (trim handles have their own arrows in TimelineClipBar).
function nudgeTrim(which: "in" | "out", direction: 1 | -1, second = false) {
	if (duration <= 0) return;
	store.pushUndoStateCoalesced(`trim-${which}`, 500);
	const delta = direction * (second ? 1 : frameStep());
	const min = minClipDuration();
	if (which === "in") {
		const next = quantizeToFrame(
			Math.max(0, Math.min(store.outPoint - min, store.inPoint + delta)),
		);
		store.trimStart = next;
	} else {
		const next = quantizeToFrame(
			Math.max(store.inPoint + min, Math.min(duration, store.outPoint + delta)),
		);
		store.trimEnd = next;
	}
}

const duration = $derived(store.metadata?.duration ?? 0);
// Normally OUTPUT time; with 'Show cut gaps' cuts get real width. Playback and export stay on the collapsed map.
const outputDuration = $derived(store.renderMap.outputDuration);
const pixelsPerSecond = $derived(
	outputDuration > 0 ? (timelineWidth * store.timelineZoom) / outputDuration : 100,
);
const totalWidth = $derived(Math.max(outputDuration * pixelsPerSecond, timelineWidth));
// Canonical axis transforms: every lane positions with `xOf` and resolves pointers with `tOf`.
const xOf = (t: number) => originalToOutput(store.renderMap, t) * pixelsPerSecond;
const tOf = (x: number) => outputToOriginal(store.renderMap, x / pixelsPerSecond);
// The playhead reads on the OUTPUT axis like the ruler; original time made the chip disagree once a cut existed.
const playheadOutput = $derived(originalToOutput(store.renderMap, store.currentTime));
const clipLeft = $derived(xOf(store.inPoint));
const clipRight = $derived(xOf(store.outPoint));
const clipWidth = $derived(Math.max(clipRight - clipLeft, 0));

// One layout per stacking lane, so rail and body share a height; a dragged card keeps its start row (see `pinnedRows`).
const laneDrag = provideLaneDrag();
let pinnedRows = $state<ReadonlyMap<string, number> | null>(null);

const zoomRowsLive = $derived(
	cardLayout(store.zoomRegions, xOf, { minWidthPx: 40, rowHeightPx: ZOOM_ROW_HEIGHT_PX }),
);
const markupRowsLive = $derived(cardLayout(store.annotations, xOf));

// Snapshot the dragged card's row once; reads the unpinned layouts untracked so it can't feed back.
$effect(() => {
	const id = laneDrag.cardId;
	if (!id) {
		pinnedRows = null;
		return;
	}
	untrack(() => {
		for (const layout of [zoomRowsLive, markupRowsLive]) {
			const card = layout.cards.find((c) => c.id === id);
			if (card) {
				pinnedRows = new Map([[id, card.row]]);
				return;
			}
		}
	});
});

const zoomLayout = $derived(
	cardLayout(store.zoomRegions, xOf, {
		minWidthPx: 40,
		rowHeightPx: ZOOM_ROW_HEIGHT_PX,
		pinnedRows: pinnedRows ?? undefined,
	}),
);
const markupLayout = $derived(
	cardLayout(store.annotations, xOf, { pinnedRows: pinnedRows ?? undefined }),
);
// Audio clips are stored in OUTPUT seconds, so they need their own projection to render-axis pixels.
const clipXOf = (outputSec: number) => store.outputToRenderSec(outputSec) * pixelsPerSecond;
const clipSpans = (clips: AudioClip[]) =>
	clips.map((c) => ({
		id: c.id,
		start: c.startOutputSec,
		end: clipEndSec(c, store.timeMap.outputDuration),
	}));
const voiceLayout = $derived(
	cardLayout(clipSpans(store.voiceClips), clipXOf, { rowHeightPx: CLIP_ROW_HEIGHT_PX }),
);
const musicLayout = $derived(
	cardLayout(clipSpans(store.musicOnlyClips), clipXOf, { rowHeightPx: CLIP_ROW_HEIGHT_PX }),
);

/** Every lane in render order: the rail and the body iterate this one list, so
 *  they can't drift out of alignment or out of order. */
const lanes = $derived(
	[
		{
			id: "audio",
			label: "Audio",
			icon: AudioLines,
			tone: "text-lane-audio",
			height: AUDIO_LANE_HEIGHT_PX,
			show: showAudioLane,
		},
		{
			id: "voice",
			label: "Voice",
			icon: Mic,
			tone: "text-lane-audio",
			height: voiceLayout.height,
			show: store.voiceClips.length > 0,
		},
		{
			id: "music",
			label: "Music",
			icon: Music2,
			tone: "text-lane-music",
			height: musicLayout.height,
			show: store.musicOnlyClips.length > 0,
		},
		{
			id: "cuts",
			label: "Cuts",
			icon: Scissors,
			tone: "text-lane-cut",
			height: CUT_LANE_HEIGHT_PX,
			show: showCutLane,
		},
		{
			id: "zoom",
			label: "Zoom",
			icon: ZoomIn,
			tone: "text-lane-zoom",
			height: zoomLayout.height,
			show: showZoomLane,
		},
		{
			id: "markup",
			label: "Markup",
			icon: Pencil,
			tone: "text-lane-markup",
			height: markupLayout.height,
			show: showMarkupLane,
		},
	].filter((l) => l.show),
);
const thumbnailWidth = $derived(
	store.thumbnailStrip.length > 0 ? Math.max(88, clipWidth / store.thumbnailStrip.length) : 112,
);
const hasTrim = $derived(duration > 0 && (store.inPoint > 0 || store.outPoint < duration));
const frameCount = $derived(
	Math.max(0, Math.round((store.metadata?.duration ?? 0) * (store.metadata?.fps ?? 0))),
);
const aspectRatioLabel = $derived.by(() => {
	const width = store.metadata?.width ?? 0;
	const height = store.metadata?.height ?? 0;
	if (!width || !height) return "Source";
	const divisor = greatestCommonDivisor(width, height);
	return `${Math.round(width / divisor)}:${Math.round(height / divisor)}`;
});

function seekToPosition(clientX: number) {
	if (!timelineEl || duration <= 0) return;
	const rect = timelineEl.getBoundingClientRect();
	const scrolled = timelineEl.scrollLeft;
	const x = clientX - rect.left + scrolled;
	// OUTPUT px → original time via the cut model (raw `x / pps` would make the playhead trail past each cut).
	const time = Math.max(0, Math.min(duration, tOf(x)));
	store.currentTime = time;
	if (videoEl) videoEl.currentTime = time;
}

function handleTimelinePointerDown(event: PointerEvent) {
	// Right/middle button is for the context menu, never seek on it.
	if (event.button !== 0) return;
	// Razor mode owns the click: place an anchor / carve a cut, never seek/drag.
	if (razorActive) {
		event.preventDefault();
		razorClickAt(event.clientX);
		return;
	}
	// Clip blocks deliberately bubble (the click must seek too), so they mark `data-selectable` to keep their selection.
	if (!(event.target as HTMLElement).closest("[data-selectable]")) {
		store.clearSelection();
	}
	isDraggingPlayhead = true;
	(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	seekToPosition(event.clientX);
}

// One rAF per frame: hover and drag-seek each forced a layout and fanned out a full `currentTime` write per event.
let pendingPointer: { x: number; y: number } | null = null;
let pointerRaf: number | null = null;
function processPointer() {
	pointerRaf = null;
	const p = pendingPointer;
	if (!p) return;
	updateHover(p.x, p.y);
	if (isDraggingPlayhead) seekToPosition(p.x);
}
function handleTimelinePointerMove(event: PointerEvent) {
	pendingPointer = { x: event.clientX, y: event.clientY };
	if (pointerRaf === null) pointerRaf = requestAnimationFrame(processPointer);
}

// A clip block bubbles pointerdown so a click still seeks, then drops the scrub if the gesture turns out to be a slip.
function cancelScrub() {
	if (pointerRaf !== null) {
		cancelAnimationFrame(pointerRaf);
		pointerRaf = null;
	}
	pendingPointer = null;
	isDraggingPlayhead = false;
}

function handleTimelinePointerUp() {
	// Land the final position now: the last queued rAF may be a frame stale, and a scrub must end where released.
	if (pointerRaf !== null) {
		cancelAnimationFrame(pointerRaf);
		pointerRaf = null;
	}
	if (pendingPointer && isDraggingPlayhead) seekToPosition(pendingPointer.x);
	pendingPointer = null;
	isDraggingPlayhead = false;
}

// Razor: two clicks carve a cut, staying armed until Esc. The tool lives in the store so every lane can decline the gesture it owns.
const razorActive = $derived(store.timelineTool === "razor");
let razorAnchor = $state<number | null>(null);

function toggleRazor() {
	store.timelineTool = razorActive ? "select" : "razor";
	razorAnchor = null;
}

// Any other edit action exits Cut, so the armed state always reflects the last action.
function disarmRazor() {
	store.timelineTool = "select";
	razorAnchor = null;
}

// Cancels a pending anchor first, then disarms; registered so the route can exit without scroller focus.
function exitTool() {
	if (razorAnchor !== null) razorAnchor = null;
	else disarmRazor();
}

// Extracted so the route can drive Home and End without the scroller holding focus.
function seekToEdge(which: "in" | "out") {
	if (duration <= 0) return;
	const t = which === "in" ? store.inPoint : Math.max(store.inPoint, store.outPoint - frameStep());
	store.currentTime = t;
	if (videoEl) videoEl.currentTime = t;
}

function splitAtPlayhead() {
	disarmRazor();
	store.splitAt(store.currentTime);
}

// Snaps to the playhead, clip in/out and region edges, falling through to the frame grid.
function razorSnap(rawOriginal: number): number {
	const targets = buildSnapTargets({
		playhead: store.currentTime,
		inPoint: store.inPoint,
		outPoint: store.outPoint,
		duration,
		regions: store.zoomRegions,
		annotations: store.annotations,
	});
	const tolerance = pixelsPerSecond > 0 ? 6 / pixelsPerSecond : 0;
	return snapTime(rawOriginal, targets, tolerance, effectiveFps()).time;
}

// Clamped, then snapped to the razor's click resolution so a cut lands on the frame preview and export use.
function clientXToOriginal(clientX: number): number {
	if (!timelineEl) return 0;
	const rect = timelineEl.getBoundingClientRect();
	const x = clientX - rect.left + timelineEl.scrollLeft;
	return razorSnap(Math.max(0, Math.min(duration, tOf(x))));
}

function razorClickAt(clientX: number) {
	const t = clientXToOriginal(clientX);
	if (razorAnchor === null) {
		razorAnchor = t;
		return;
	}
	const lo = Math.min(razorAnchor, t);
	const hi = Math.max(razorAnchor, t);
	razorAnchor = null;
	// addCut drops sub-10ms ranges; merge folds it into any neighbour.
	if (store.addCut(lo, hi, "manual")) store.mergeCuts();
}

// Hover-scrub: a decoded frame thumbnail follows the cursor, with the output timecode under it.
let hover = $state<{
	clientX: number;
	clientY: number;
	top: number;
	outputSec: number;
	originalSec: number;
} | null>(null);
// A storyboard sprite cell (one decode, then CSS crops); `previewAt` only covers the moment before it is ready.
const HOVER_PREVIEW_H = 64;
const hoverCell = $derived.by(() => {
	void filmstripVersion;
	if (!hover || !tileProvider) return undefined;
	const sb = tileProvider.storyboard();
	if (!sb || sb.count <= 0 || sb.durationSec <= 0 || sb.cellH <= 0) {
		return undefined;
	}
	return { url: sb.url, ...storyboardCrop(sb, hover.originalSec, HOVER_PREVIEW_H) };
});
const hoverUrl = $derived.by(() => {
	void filmstripVersion;
	if (!hover || !tileProvider || hoverCell) return undefined;
	return tileProvider.previewAt(hover.originalSec);
});

// Last resort: the nearest coarse Rust strip frame, because an empty grey box is worse than a rough one.
const hoverStripUrl = $derived.by(() => {
	if (!hover || hoverCell || hoverUrl) return undefined;
	const strip = store.thumbnailStrip;
	if (strip.length === 0 || duration <= 0) return undefined;
	const i = Math.min(
		strip.length - 1,
		Math.max(0, Math.floor((hover.originalSec / duration) * strip.length)),
	);
	return strip[i];
});

// Snapped end of the live razor span (for the preview band) while armed.
const razorHoverTime = $derived.by(() => {
	if (!razorActive || !hover) return null;
	return razorSnap(Math.max(0, Math.min(duration, hover.originalSec)));
});

function updateHover(clientX: number, clientY = 0) {
	if (!timelineEl || isDraggingPlayhead || duration <= 0) {
		hover = null;
		return;
	}
	const rect = timelineEl.getBoundingClientRect();
	const xInViewport = clientX - rect.left;
	if (xInViewport < 0 || xInViewport > rect.width) {
		hover = null;
		return;
	}
	const outputSec = clientXToOutput(clientX);
	hover = {
		clientX,
		clientY,
		top: rect.top,
		outputSec,
		originalSec: outputToOriginal(store.renderMap, outputSec),
	};
}
function clearHover() {
	hover = null;
}

function handleTimelineKeydown(event: KeyboardEvent) {
	if (duration <= 0) return;

	const mod = event.ctrlKey || event.metaKey;

	// Razor (Cut) tool: C arms/disarms; Esc cancels a pending anchor, else disarms.
	if (event.key === "Escape" && razorActive) {
		event.preventDefault();
		exitTool();
		return;
	}
	if ((event.key === "c" || event.key === "C") && !mod) {
		event.preventDefault();
		toggleRazor();
		return;
	}

	// Paste works anywhere in the timeline (cards own copy/duplicate, so they need focus).
	if (mod && (event.key === "v" || event.key === "V")) {
		if (zoomClipboard) {
			event.preventDefault();
			pasteRegion();
		}
		return;
	}

	// Bail on Ctrl/Cmd so a global combo (⌘K/⌘J/⌘S) doesn't also fire a single-letter transport here.
	if (mod) return;

	const step = event.shiftKey ? 1 : frameStep();

	if (event.key === "ArrowLeft" && !event.altKey) {
		event.preventDefault();
		const next = quantizeToFrame(Math.max(0, store.currentTime - step));
		store.currentTime = next;
		if (videoEl) videoEl.currentTime = next;
	}

	if (event.key === "ArrowRight" && !event.altKey) {
		event.preventDefault();
		const next = quantizeToFrame(Math.min(duration, store.currentTime + step));
		store.currentTime = next;
		if (videoEl) videoEl.currentTime = next;
	}

	// Premiere-style in/out point shortcuts.
	if (event.key === "i" || event.key === "I") {
		event.preventDefault();
		if (event.shiftKey) {
			store.pushUndoState();
			store.trimStart = 0;
		} else {
			setTrimPoint("in");
		}
	}
	if (event.key === "o" || event.key === "O") {
		event.preventDefault();
		if (event.shiftKey) {
			store.pushUndoState();
			store.trimEnd = duration;
		} else {
			setTrimPoint("out");
		}
	}

	// Match `event.code`: shifted brackets become other characters on some layouts.
	if (event.altKey && event.code === "BracketLeft") {
		event.preventDefault();
		nudgeTrim("in", 1, event.shiftKey);
	}
	if (event.altKey && event.code === "BracketRight") {
		event.preventDefault();
		nudgeTrim("out", -1, event.shiftKey);
	}

	// Home/End jump the playhead to the in/out points (NLE convention).
	if (event.key === "Home") {
		event.preventDefault();
		seekToEdge("in");
	}
	if (event.key === "End") {
		event.preventDefault();
		seekToEdge("out");
	}

	// Split the clip at the playhead ("S").
	if (event.key === "s" || event.key === "S") {
		event.preventDefault();
		splitAtPlayhead();
	}

	// Delete is NOT handled here: it is a document-level command over the selection, owned by the editor page.

	// J/K/L transport (see shuttle state above).
	if (event.key === "k" || event.key === "K") {
		event.preventDefault();
		shuttleDirection = 0;
		shuttleSpeedIndex = 0;
		if (videoEl) videoEl.pause();
		store.isPlaying = false;
	}
	if (event.key === "l" || event.key === "L") {
		event.preventDefault();
		if (shuttleDirection === 1) {
			shuttleSpeedIndex = Math.min(SHUTTLE_SPEEDS.length - 1, shuttleSpeedIndex + 1);
		} else {
			shuttleDirection = 1;
			shuttleSpeedIndex = 0;
		}
		if (videoEl) {
			videoEl.playbackRate = SHUTTLE_SPEEDS[shuttleSpeedIndex] * playbackSpeed;
			void videoEl.play();
		}
		store.isPlaying = true;
	}
	if (event.key === "j" || event.key === "J") {
		event.preventDefault();
		if (videoEl) videoEl.pause();
		store.isPlaying = false;
		if (shuttleDirection === -1) {
			shuttleSpeedIndex = Math.min(SHUTTLE_SPEEDS.length - 1, shuttleSpeedIndex + 1);
		} else {
			shuttleDirection = -1;
			shuttleSpeedIndex = 0;
		}
	}
}

// A persisted zoom or a window resize can land outside the legal ceiling, so pull it back.
$effect(() => {
	if (outputDuration <= 0 || timelineWidth <= 0) return;
	const legal = clampTimelineZoom(store.timelineZoom, outputDuration, timelineWidth);
	if (legal !== store.timelineZoom) store.timelineZoom = legal;
});

function handleResize() {
	if (!timelineEl) return;
	timelineWidth = timelineEl.clientWidth;
}

// Hiding a lane can make the browser clamp scrollTop to 0, so resync or the rail keeps a stale offset.
$effect(() => {
	// biome-ignore lint/suspicious/noUnusedExpressions: the bare read is the effect's dependency.
	lanes.length;
	untrack(() => handleScroll());
});

function handleScroll() {
	if (!timelineEl) return;
	scrollLeft = timelineEl.scrollLeft;
	// Written straight to the node: a reactive round-trip lands a frame late and shears the labels off their lanes.
	if (railInnerEl) railInnerEl.style.transform = `translateY(${-timelineEl.scrollTop}px)`;
}

function handleTimelineWheel(event: WheelEvent) {
	if (!timelineEl) return;

	const canScrollVertically = timelineEl.scrollHeight - timelineEl.clientHeight > 1;
	const intent = wheelIntent(event, canScrollVertically);

	// The scroller owns both axes, so a vertical notch is left to the browser; `handleScroll` still carries the rail.
	if (intent.kind === "none" || intent.kind === "vertical") return;

	if (intent.kind === "zoom") {
		event.preventDefault();
		const rect = timelineEl.getBoundingClientRect();
		const anchorX = event.clientX - rect.left;
		// Anchor in OUTPUT seconds so the point under the cursor stays put across the zoom.
		const anchorOut = duration > 0 ? (timelineEl.scrollLeft + anchorX) / pixelsPerSecond : 0;
		// Multiplicative, so one notch covers the same proportion whether the clip is 10 seconds or 30 minutes.
		const nextZoom = clampTimelineZoom(
			store.timelineZoom * (intent.direction > 0 ? 1.12 : 1 / 1.12),
			outputDuration,
			timelineWidth,
		);
		if (nextZoom === store.timelineZoom) return;
		store.timelineZoom = nextZoom;
		requestAnimationFrame(() => {
			if (!timelineEl || outputDuration <= 0) return;
			const nextPixelsPerSecond = (timelineEl.clientWidth * nextZoom) / outputDuration;
			timelineEl.scrollLeft = Math.max(0, anchorOut * nextPixelsPerSecond - anchorX);
		});
		return;
	}

	event.preventDefault();
	timelineEl.scrollLeft += intent.delta;
}

function syncVideoTime() {
	if (!videoEl) return;
	videoEl.currentTime = Math.max(0, Math.min(duration, store.currentTime));
}

function addFocusRegion() {
	if (duration <= 0) return;
	disarmRazor();
	const start = Math.max(store.inPoint, store.currentTime - 0.35);
	const end = Math.min(store.outPoint, Math.max(start + 0.8, store.currentTime + 0.85));
	store.addZoomRegion(start, end, 1.8);
}

function setTrimPoint(kind: "in" | "out") {
	if (duration <= 0) return;
	disarmRazor();
	store.pushUndoState();
	const min = minClipDuration();
	if (kind === "in") {
		const nextIn = quantizeToFrame(Math.min(store.currentTime, Math.max(0, store.outPoint - min)));
		store.trimStart = nextIn;
		if (store.currentTime < nextIn) store.currentTime = nextIn;
	} else {
		const nextOut = quantizeToFrame(
			Math.max(store.currentTime, Math.min(duration, store.inPoint + min)),
		);
		store.trimEnd = nextOut;
		if (store.currentTime > nextOut) store.currentTime = nextOut;
	}
	syncVideoTime();
}

// Editable fields only: id/source are regenerated on paste so it never collides with an existing region.
type ZoomClipboard = Omit<ZoomRegion, "id" | "source">;
let zoomClipboard = $state<ZoomClipboard | null>(null);

function snapshotForClipboard(r: ZoomRegion): ZoomClipboard {
	return {
		start: r.start,
		end: r.end,
		scale: r.scale,
		easeIn: { ...r.easeIn },
		easeOut: { ...r.easeOut },
		rampIn: r.rampIn,
		rampOut: r.rampOut,
		centerX: r.centerX,
		centerY: r.centerY,
		motionBlur: r.motionBlur,
	};
}

function copyRegion(r: ZoomRegion) {
	zoomClipboard = snapshotForClipboard(r);
}

// Place a region at `startAt`, copying the rest from `template`.
function placeRegion(template: ZoomClipboard, startAt: number) {
	if (duration <= 0) return;
	const span = template.end - template.start;
	const start = Math.max(0, Math.min(duration - span, startAt));
	const end = start + span;
	// addZoomRegion only seeds geometry/scale; layer the rest on so the copy matches the source.
	const id = store.addZoomRegion(start, end, template.scale, {
		x: template.centerX,
		y: template.centerY,
	});
	store.updateZoomRegion(id, {
		easeIn: { ...template.easeIn },
		easeOut: { ...template.easeOut },
		rampIn: template.rampIn,
		rampOut: template.rampOut,
		motionBlur: template.motionBlur,
	});
}

function duplicateRegion(r: ZoomRegion) {
	const span = r.end - r.start;
	// Offset by min(0.25s, span) so the copy sits visibly right without overshooting.
	const offset = Math.min(0.25, span);
	placeRegion(snapshotForClipboard(r), r.start + offset);
}

// Store nudges the geometry diagonally; we add a +0.25s time shift on top.
function duplicateAnnotation(annotation: import("../stores/editor-store.svelte").Annotation) {
	if (duration <= 0) return;
	const dup = store.duplicateAnnotation(annotation.id);
	if (!dup) return;
	const span = dup.end - dup.start;
	const offset = Math.min(0.25, span);
	const nextStart = Math.max(0, Math.min(duration - span, dup.start + offset));
	store.updateAnnotation(dup.id, {
		start: nextStart,
		end: nextStart + span,
	});
}

function pasteRegion() {
	if (!zoomClipboard) return;
	const span = zoomClipboard.end - zoomClipboard.start;
	placeRegion(zoomClipboard, store.currentTime - span * 0.5);
}

function resetTrim() {
	disarmRazor();
	store.pushUndoState();
	store.trimStart = 0;
	store.trimEnd = duration;
	syncVideoTime();
}

onMount(() => {
	handleResize();
	const observer = new ResizeObserver(handleResize);
	if (timelineEl) observer.observe(timelineEl);
	// Driven by the route handler so the toolbar keycaps stay honest without scroller focus; unregistered on unmount.
	const offCommands = store.registerTimelineCommands({
		splitAtPlayhead,
		toggleRazor,
		exitTool,
		trimToPlayhead: setTrimPoint,
		seekToEdge,
	});
	return () => {
		observer.disconnect();
		offCommands();
		if (pointerRaf !== null) cancelAnimationFrame(pointerRaf);
	};
});
</script>

<!-- Track-header row for the fixed left rail: full-width icon + label pill, the
     tone colour carried by the icon so every row header reads the same. -->
{#snippet railLabel(Icon: typeof Video, label: string, iconClass: string)}
  <span
    class="inline-flex w-full items-center justify-start gap-1.5 rounded-md px-1.5 py-1.5 text-[8.5px] font-semibold uppercase leading-none tracking-wide text-muted-foreground"
  >
    <Icon class="size-3 shrink-0 {iconClass}" />
    <span class="truncate">{label}</span>
  </span>
{/snippet}

<!-- Fills whatever height the editor gives it (the panel is user-resizable), so
     the toolbar stays pinned and the tracks scroll inside the rest. -->
<div
  class="flex h-full min-h-0 select-none flex-col border-t border-border/60 bg-card/30 px-2 pt-1.5 pb-2"
>
  <div class="shrink-0">
  <TimelineToolbar
    {store}
    fps={effectiveFps()}
    {hasTrim}
    {aspectRatioLabel}
    {frameCount}
    {playbackSpeed}
    speeds={SPEEDS}
    {timeMode}
    hasSelectedRegion={hasFramableSelection}
    {razorActive}
    {showAudioLane}
    {showZoomLane}
    {showMarkupLane}
    {showCutLane}
    showCutGaps={store.showCutGaps}
    onSetTrim={setTrimPoint}
    onSplit={splitAtPlayhead}
    onToggleRazor={toggleRazor}
    onAddFocusRegion={addFocusRegion}
    onResetTrim={resetTrim}
    onZoomTimeline={zoomTimeline}
    onSelectSpeed={(speed) => (playbackSpeed = speed)}
    onSetTimeMode={(mode) => (store.timeMode = mode)}
    onZoomToFit={zoomToFit}
    onZoomToSelection={zoomToSelection}
    onToggleAudioLane={() => (showAudioLane = !showAudioLane)}
    onToggleZoomLane={() => (zoomLanePref = !showZoomLane)}
    onToggleMarkupLane={() => (markupLanePref = !showMarkupLane)}
    onToggleCutLane={() => (showCutLane = !showCutLane)}
    onToggleCutGaps={() => (store.showCutGaps = !store.showCutGaps)}
  />
  </div>

  <!-- Frozen row and column, the way a spreadsheet does it: the rail is a
       sibling of the horizontal scroller so lane names never move sideways (and
       never overlap a card at t≈0), and the ruler is `sticky top-0` inside the
       scroller so it holds while the lanes scroll under it. The scroller owns
       BOTH axes for that to work — sticky resolves against the nearest scrolling
       ancestor, so a separate outer y-scroller would leave the ruler pinned to
       content that never moves. The rail has no scrollbar of its own; it follows
       the track's scrollTop, which keeps every label on its lane's row. -->
  <div
    class="flex min-h-0 flex-1 overflow-hidden rounded-xl border border-border/60 bg-background/60 shadow-(--shadow-craft-inset)"
  >
    <div
      class="relative z-10 flex w-18 shrink-0 flex-col border-r border-border/60 bg-card/50"
    >
      <!-- Corner cell: holds the rail's edge against the sticky ruler. -->
      <div class="h-7 shrink-0 border-b border-border/60"></div>
      <div class="min-h-0 flex-1 overflow-hidden">
        <!-- Track headers. Each row's height comes from the same `lanes` entry the
             body uses, so a lane that grows takes its label with it. -->
        <div bind:this={railInnerEl} class="px-1 pb-2 pt-1.5 will-change-transform">
          <div
            class="flex items-center justify-center"
            style="height: {CLIP_LANE_HEIGHT_PX}px;"
          >
            {@render railLabel(Video, "Clip", "text-foreground/70")}
          </div>
          {#each lanes as lane (lane.id)}
            <div
              class="flex items-start justify-center"
              style="height: {lane.height}px; margin-top: {LANE_GAP}px;"
            >
              {@render railLabel(lane.icon, lane.label, lane.tone)}
            </div>
          {/each}
        </div>
      </div>
    </div>

    <div
      bind:this={timelineEl}
      role="slider"
      tabindex="0"
      aria-label="Timeline scrubber"
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={store.currentTime}
      class="custom-scrollbar relative min-w-0 flex-1 overflow-auto overscroll-contain rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/60"
      style={razorActive ? "cursor: none" : ""}
      onpointerdown={handleTimelinePointerDown}
      onpointermove={handleTimelinePointerMove}
      onpointerup={handleTimelinePointerUp}
      onpointercancel={handleTimelinePointerUp}
      onwheel={handleTimelineWheel}
      onscroll={handleScroll}
      onpointerleave={clearHover}
      onkeydown={handleTimelineKeydown}
    >
      <div class="relative min-w-full" style="width: {totalWidth}px;">
        <!-- Opaque, or the lanes scrolling underneath show through the ticks.
             z-20 puts it over the cards but under the playhead (z-30), so the
             head still reads as crossing the ruler band. -->
        <div class="sticky top-0 z-20 bg-background">
          <TimelineRuler
            duration={outputDuration}
            {pixelsPerSecond}
            {timeMode}
            fps={effectiveFps()}
            viewportLeftPx={scrollLeft}
            viewportWidthPx={timelineWidth}
          />
        </div>

      <!-- No horizontal padding: lanes must share the x-origin of the ruler and
           playhead (both direct children at x=0), or every tile sits offset from
           the ticks and the playhead line.
           `inert` sits here, not on the whole timeline: every structural gesture
           lives inside this stack, while scrub/zoom/scroll belong to the
           scroller above, so a write-locked timeline stays watchable. -->
      <div class="relative pb-2 pt-1.5" inert={readOnly}>
        <TimelineClipBar
          {store}
          {videoEl}
          fps={effectiveFps()}
          {duration}
          {pixelsPerSecond}
          {clipLeft}
          {clipWidth}
          {thumbnailWidth}
          {timeMode}
          {clientXToOutput}
          onSpineGesture={cancelScrub}
          {tileProvider}
          {filmstripVersion}
          viewportLeftPx={Math.max(0, scrollLeft - LANE_PAD)}
          viewportWidthPx={timelineWidth}
        />

        <!-- Same `lanes` list as the rail: same order, same visibility, same
             heights. Cuts sit next to Audio because cutting against the waveform
             is the common task — by adjacency, not by the cut lane drawing its
             own copy of the envelope. The waveform belongs to the Audio lane. -->
        {#each lanes as lane (lane.id)}
          {#if lane.id === "audio"}
            <TimelineAudioLane {store} {pixelsPerSecond} {duration} />
          {:else if lane.id === "voice"}
            <TimelineMusicLane
              {store}
              clips={store.voiceClips}
              {pixelsPerSecond}
              layout={voiceLayout}
              variant="voice"
              panelTab="audio"
            />
          {:else if lane.id === "music"}
            <TimelineMusicLane
              {store}
              clips={store.musicOnlyClips}
              {pixelsPerSecond}
              layout={musicLayout}
              variant="music"
              panelTab="music"
            />
          {:else if lane.id === "cuts"}
            <TimelineCutLane {store} {pixelsPerSecond} {duration} fps={effectiveFps()} />
          {:else if lane.id === "zoom"}
            <TimelineZoomLane
              {store}
              {pixelsPerSecond}
              fps={effectiveFps()}
              {duration}
              {timeMode}
              layout={zoomLayout}
              onCopy={copyRegion}
              onDuplicate={duplicateRegion}
            />
          {:else if lane.id === "markup"}
            <TimelineAnnotationLane
              {store}
              {pixelsPerSecond}
              fps={effectiveFps()}
              {duration}
              {timeMode}
              layout={markupLayout}
              onDuplicate={duplicateAnnotation}
            />
          {/if}
        {/each}
      </div>

      <TimelinePlayhead
        outputTime={playheadOutput}
        leftPx={playheadOutput * pixelsPerSecond}
        fps={effectiveFps()}
        isDragging={isDraggingPlayhead}
        isPlaying={store.isPlaying}
        {timeMode}
      />

      <!-- Razor preview: a hairline at the pending click point, and once an anchor
           is set, the destructive span that the second click will remove. -->
      {#if razorActive && hover}
        {@const endT = razorHoverTime ?? hover.originalSec}
        {@const anchorX = razorAnchor !== null ? xOf(razorAnchor) : xOf(endT)}
        {@const hoverX = xOf(endT)}
        {#if razorAnchor !== null}
          {@const left = Math.min(anchorX, hoverX)}
          {@const w = Math.abs(hoverX - anchorX)}
          <div
            class="pointer-events-none absolute inset-y-0 z-20 border-x border-lane-cut/70 bg-lane-cut/15"
            style="left: {left}px; width: {w}px; background-image: repeating-linear-gradient(45deg, transparent, transparent 5px, color-mix(in srgb, var(--lane-cut) 20%, transparent) 5px, color-mix(in srgb, var(--lane-cut) 20%, transparent) 10px);"
          >
            {#if w > 36}
              <span
                class="absolute left-1/2 top-1 -translate-x-1/2 whitespace-nowrap rounded bg-lane-cut px-1 py-0.5 font-mono text-[9px] font-bold text-background shadow-sm"
              >
                âˆ’{Math.abs(endT - razorAnchor).toFixed(2)}s
              </span>
            {/if}
          </div>
        {/if}
        <div
          class="pointer-events-none absolute inset-y-0 z-20 w-px bg-lane-cut"
          style="left: {anchorX}px;"
        ></div>
      {/if}
      </div>
    </div>
  </div>
</div>

<!-- Scissor cursor for the razor tool: the scroller hides its native cursor
     (cursor:none) and this glyph rides the pointer instead, so the cursor
     literally reads as a scissor while armed. -->
{#if razorActive && hover}
  <div
    class="pointer-events-none fixed z-50 -translate-x-1/2 -translate-y-1/2 text-lane-cut drop-shadow-md"
    style="left: {hover.clientX}px; top: {hover.clientY}px;"
  >
    <Scissors class="size-5" />
  </div>
{/if}

<!-- Hover-scrub preview: fixed so it floats above the timeline without being
     clipped by the scroller's overflow. Only with the WebCodecs filmstrip. -->
{#if hover && !isDraggingPlayhead && !razorActive && (tileProvider || hoverStripUrl)}
  <div
    class="pointer-events-none fixed z-50 flex -translate-x-1/2 -translate-y-full flex-col items-center gap-1"
    style="left: {hover.clientX}px; top: {hover.top - 8}px;"
  >
    <div
      class="overflow-hidden rounded-md border border-border/70 bg-card shadow-lg"
    >
      {#if hoverCell}
        <!-- One cell of the storyboard sprite, cropped via background-position. -->
        <div
          class="h-16"
          style="width: {hoverCell.dispW}px; background-image: url('{hoverCell.url}'); background-repeat: no-repeat; background-size: {hoverCell.bgW}px {hoverCell.bgH}px; background-position: -{hoverCell.offX}px -{hoverCell.offY}px;"
        ></div>
      {:else if hoverUrl}
        <img
          src={hoverUrl}
          alt=""
          class="block h-16 w-auto object-cover"
          draggable="false"
        />
      {:else if hoverStripUrl}
        <img
          src={hoverStripUrl}
          alt=""
          class="block h-16 w-auto object-cover"
          draggable="false"
        />
      {:else}
        <div class="h-16 w-28 animate-pulse bg-muted/60"></div>
      {/if}
    </div>
    <span
      class="rounded bg-popover px-1.5 py-0.5 font-mono text-[10px] tabular-nums text-foreground shadow-sm"
    >
      {formatTimeByMode(hover.outputSec, timeMode, effectiveFps())}
    </span>
  </div>
{/if}

<style>
  /* Width applies to the vertical bar, shown once the lanes outgrow the panel. */
  .custom-scrollbar::-webkit-scrollbar {
    height: 8px;
    width: 8px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--color-foreground) 14%, transparent);
    border-radius: 999px;
    transition: background 0.2s cubic-bezier(0.625, 0.05, 0, 1);
  }

  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: color-mix(in srgb, var(--color-foreground) 24%, transparent);
  }

  .custom-scrollbar {
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--color-foreground) 14%, transparent)
      transparent;
  }
</style>
