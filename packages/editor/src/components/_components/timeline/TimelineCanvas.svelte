<script lang="ts">
import {
	Captions,
	ChevronDown,
	ChevronRight,
	Clock,
	Copy,
	Ellipsis,
	Eye,
	EyeOff,
	Film,
	Gauge,
	Highlighter,
	Lock,
	Maximize2,
	Mic,
	Pause,
	Play,
	Repeat,
	Scissors,
	SquareSplitHorizontal,
	Sun,
	Trash2,
	Unlock,
	Upload,
	Volume2,
	VolumeOff,
	ZoomIn,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as ContextMenu from "@recast/ui/context-menu";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { onMount, untrack } from "svelte";
import { kindLabel } from "../../../lib/annotations/kind-label";
import { clipEndSec } from "../../../lib/audio/music";
import { formatTimeByMode, frameStepOutput } from "../../../lib/editor/time";
import { visibleTicks } from "../../../lib/timeline/canvas-ruler";
import {
	clampScroll,
	DEFAULT_RESOLUTION,
	frameToX,
	scrollByPixels,
	type TimelineView,
	xToFrame,
	zoomAt,
} from "../../../lib/timeline/canvas-view";
import { type FilmstripBlock, planFilmstrip } from "../../../lib/timeline/filmstrip";
import type { Storyboard, TileProvider } from "../../../lib/timeline/filmstrip-source";
import { originalToOutput, outputToOriginal } from "../../../lib/timeline/time-map";
import {
	buildTimelineRows,
	type ClipKind,
	type TimelineClip,
	type TimelineRow,
} from "../../../lib/timeline/view-model";
import type { EditorStore } from "../../../stores/editor-store.svelte";
import { effectiveFps } from "./timeline-helpers";

interface Props {
	store: EditorStore;
	/** The preview element, so the header transport can play/pause it. */
	videoEl?: HTMLVideoElement | null;
	/** Loop flag; the editor page owns the seek-and-replay, this only flips it. */
	loopEnabled?: boolean;
	/** Filmstrip source for the video clip; null on hosts without one. */
	tileProvider?: TileProvider | null;
	/** Bumped by the host when new thumbnails decode, to force a repaint. */
	filmstripVersion?: number;
}
let {
	store,
	videoEl = null,
	loopEnabled = $bindable(false),
	tileProvider = null,
	filmstripVersion = 0,
}: Props = $props();

const RULER_HEIGHT = 36;
const RULER_LABEL_Y = 18;
const RULER_TICK_MAJOR = 9;
const RULER_TICK_MINOR = 3;
// Uniform lane height for every kind (Diffusion's DEFAULT_CLIP_HEIGHT); rows sit
// nearly contiguous the way its layer list does.
const ROW_H = 40;
const WAVE_BAND = 22;
const SUBROW_H = 30;
const ROW_GAP = 2;
// The video lane splits picture/waveform only when tall enough for both.
const VIDEO_WAVE_MIN_H = 62;
const CLIP_RADIUS = 4;
const CLIP_LABEL_X = 6;
const CLIP_LABEL_HEIGHT = 18;
const TRIM_HANDLE_PX = 10;
const SNAP_PX = 10;
const MIN_CLIP_FRAMES = 2;
// How near the playhead line a press counts as grabbing it (over the lanes).
const PLAYHEAD_GRAB_PX = 5;
const TRACK_HEADER_W = 240;
const SAMPLE_WIDTH = 2;
const FILMSTRIP_TILE_HEIGHT = 48;
const FILMSTRIP_TILE_WIDTH = 72;
// The header is a flex sibling, so the canvas owns only the content region. A
// small left pad keeps frame 0 off the edge (Diffusion's TIMELINE_PADDING_LEFT).
const GUTTER = 8;

function rowHeight(_kind: ClipKind): number {
	return ROW_H;
}

const ZOOM_SENSITIVITY = 0.01;
const ZOOM_DELTA_CLAMP = 50;
const SCROLL_X_SENSITIVITY = 2;

// Diffusion Studio's playhead knob, verbatim: a 10×13 downward pin.
const KNOB_PATH = new Path2D(
	"M6.47987 12.372C5.68637 13.2449 4.31359 13.2449 3.52004 12.372L0.520121 9.07213C0.185444 8.70399 4.73995e-07 8.22432 4.61669e-07 7.72679L3.19793e-07 2C2.92428e-07 0.89543 0.895431 1.57728e-07 2 2.54292e-07L7.99987 7.78817e-07C9.10449 8.75386e-07 9.99994 0.895511 9.99987 2.00013L9.99949 7.72692C9.99946 8.22437 9.81405 8.70396 9.47945 9.07206L6.47987 12.372Z",
);

const LANE_ICONS: Record<ClipKind, typeof Film> = {
	video: Film,
	zoom: ZoomIn,
	markup: Highlighter,
	caption: Captions,
	audio: Mic,
};

// The one keyframe-able property each kind exposes today. Absent = the row does
// not expand. Keyframes are a follow-up; these edit the clip's current value.
interface PropSpec {
	label: string;
	icon: typeof Film;
	min: number;
	max: number;
	step: number;
	format: (v: number) => string;
}
const PROP: Partial<Record<ClipKind, PropSpec>> = {
	video: {
		label: "Speed",
		icon: Gauge,
		min: 0.25,
		max: 4,
		step: 0.05,
		format: (v) => `${v.toFixed(2)}×`,
	},
	audio: {
		label: "Volume",
		icon: Volume2,
		min: 0,
		max: 200,
		step: 1,
		format: (v) => `${Math.round(v)}%`,
	},
	zoom: {
		label: "Scale",
		icon: ZoomIn,
		min: 1,
		max: 4,
		step: 0.1,
		format: (v) => `${v.toFixed(1)}×`,
	},
	markup: {
		label: "Opacity",
		icon: Sun,
		min: 0,
		max: 1,
		step: 0.05,
		format: (v) => `${Math.round(v * 100)}%`,
	},
	caption: {
		label: "Size",
		icon: Captions,
		min: 2,
		max: 8,
		step: 0.1,
		format: (v) => `${v.toFixed(1)}%`,
	},
};

let containerEl: HTMLDivElement | undefined = $state();
let canvasEl: HTMLCanvasElement | undefined = $state();
let ctx: CanvasRenderingContext2D | null = null;

let cssW = $state(900);
let cssH = $state(200);
let dpr = 1;
// Once the user zooms/pans, stop auto-fitting on resize and let them drive.
let userAdjusted = false;

// Width available for the time content, i.e. everything right of the header gutter.
const contentW = $derived(Math.max(1, cssW - GUTTER));

let view = $state<TimelineView>({
	scrollFrames: 0,
	resolution: DEFAULT_RESOLUTION,
});
let scrollY = $state(0);
let expandedRows = $state(new Set<string>());

const fps = $derived(effectiveFps(store.metadata?.fps));
const outputDurationSec = $derived(store.renderMap.outputDuration);
const totalFrames = $derived(Math.max(0, outputDurationSec * fps));
const playheadFrame = $derived(
	Math.max(0, originalToOutput(store.renderMap, store.currentTime) * fps),
);
const videoTrimmable = $derived(store.segments.length === 1);
const videoName = $derived(store.videoPath.split(/[\\/]/).pop() || "Video");

const rows = $derived.by<TimelineRow[]>(() => {
	const selClip = store.selectedClipStart;
	const outDur = store.timeMap.outputDuration;
	return buildTimelineRows({
		fps,
		map: store.renderMap,
		videoName,
		segments: store.segments.map((s) => ({
			id: String(s.start),
			start: s.start,
			end: s.end,
			label: "Video",
			selected: selClip !== null && Math.abs(s.start - selClip) < 1e-4,
		})),
		zoomRegions: store.zoomRegions.map((z) => ({
			id: z.id,
			start: z.start,
			end: z.end,
			label: `${z.scale.toFixed(1)}×`,
			selected: store.selectedZoomRegionId === z.id,
			hidden: z.hidden,
		})),
		annotations: store.annotations.map((a) => ({
			id: a.id,
			start: a.start,
			end: a.end,
			label: kindLabel(a),
			selected: store.selectedAnnotationId === a.id,
			hidden: a.hidden,
			locked: a.locked,
		})),
		captions: (store.captionTranscript?.segments ?? []).map((s) => ({
			id: s.id,
			start: s.start,
			end: s.end,
			label: s.text?.trim() || "Caption",
		})),
		voiceClips: store.voiceClips.map((c) => ({
			id: c.id,
			start: c.startOutputSec,
			end: clipEndSec(c, outDur),
			label: "Voice",
			selected: store.selectedMusicClipId === c.id,
		})),
		musicClips: store.musicOnlyClips.map((c) => ({
			id: c.id,
			start: c.startOutputSec,
			end: clipEndSec(c, outDur),
			label: "Music",
			selected: store.selectedMusicClipId === c.id,
		})),
	});
});

interface LaidRow {
	row: TimelineRow;
	top: number;
	/** Clip-band height (excludes the property sub-row). */
	height: number;
	expandable: boolean;
	expanded: boolean;
}
const rowLayout = $derived.by<LaidRow[]>(() => {
	let top = ROW_GAP;
	const out: LaidRow[] = [];
	for (const row of rows) {
		const expandable = PROP[row.kind] !== undefined && row.clips.length >= 1;
		const expanded = expandable && expandedRows.has(row.id);
		const height = rowHeight(row.kind);
		out.push({ row, top, height, expandable, expanded });
		top += height + (expanded ? SUBROW_H : 0) + ROW_GAP;
	}
	return out;
});
const contentHeight = $derived(
	rowLayout.reduce((h, l) => h + l.height + (l.expanded ? SUBROW_H : 0) + ROW_GAP, ROW_GAP),
);
const allClips = $derived(rows.flatMap((r) => r.clips));

// Cut seams (silence removed): each collapses to one OUTPUT frame where its
// original range folds shut. Drawn as a full-height notch, not a clip.
const cutFrames = $derived.by<number[]>(() => {
	if (!store.cutsEnabled) return [];
	return store.cuts.map((c) => originalToOutput(store.renderMap, c.start) * fps);
});

// Guarded like the rest of the editor (Editor.svelte); never touch a bare
// localStorage, which throws under SSR and in privacy modes.
const EXPAND_KEY = "recast:tl-expanded";
const tlStorage = typeof localStorage === "undefined" ? null : localStorage;
function persistExpanded(ids: Set<string>) {
	try {
		tlStorage?.setItem(EXPAND_KEY, JSON.stringify([...ids]));
	} catch {
		// best-effort; expansion just won't persist.
	}
}
function loadExpanded(): Set<string> {
	try {
		const raw = tlStorage?.getItem(EXPAND_KEY);
		if (raw) return new Set(JSON.parse(raw) as string[]);
	} catch {
		// best-effort; start with nothing expanded.
	}
	return new Set();
}
function toggleExpand(id: string) {
	const next = new Set(expandedRows);
	if (next.has(id)) next.delete(id);
	else next.add(id);
	expandedRows = next;
	persistExpanded(next);
	scrollY = clampScrollY(scrollY);
}

// The property edits the SELECTED clip of a multi-clip row, or the first one.
function propClipId(row: TimelineRow): string | undefined {
	return (row.clips.find((c) => c.selected) ?? row.clips[0])?.id;
}
function rowSelected(row: TimelineRow): boolean {
	return row.clips.some((c) => c.selected);
}
// The lane-level toggles that map cleanly to the store: markup hides, audio
// mutes (gain 0). Other kinds have none, so their row shows no toggle.
function rowToggleKind(row: TimelineRow): "hide" | "mute" | null {
	if (row.kind === "markup") return "hide";
	if (row.kind === "audio") return "mute";
	return null;
}
function rowToggled(row: TimelineRow): boolean {
	if (row.kind === "markup") return store.annotationsGloballyHidden;
	if (row.kind === "audio") return propValue(row) <= 0;
	return false;
}
function toggleRow(row: TimelineRow) {
	if (row.kind === "markup") {
		store.annotationsGloballyHidden = !store.annotationsGloballyHidden;
	} else if (row.kind === "audio") {
		const id = propClipId(row);
		if (id) store.updateMusicClip(id, { gain: rowToggled(row) ? 100 : 0 });
	}
}
function propValue(row: TimelineRow): number {
	const id = propClipId(row);
	if (row.kind === "video") return store.segmentSpeedAt(Number(id));
	if (row.kind === "caption") return store.captionStyle.fontSizePct;
	if (row.kind === "audio") return store.musicClips.find((c) => c.id === id)?.gain ?? 100;
	if (row.kind === "zoom") return store.zoomRegions.find((z) => z.id === id)?.scale ?? 1;
	if (row.kind === "markup") return store.annotations.find((a) => a.id === id)?.opacity ?? 1;
	return 0;
}
function setProp(row: TimelineRow, v: number) {
	// Caption size is a global style, so it needs no clip id; the rest edit a clip.
	if (row.kind === "caption") return store.updateCaptionStyle({ fontSizePct: v });
	const id = propClipId(row);
	if (!id) return;
	if (row.kind === "video") store.setSegmentSpeed(Number(id), v);
	else if (row.kind === "audio") store.updateMusicClip(id, { gain: v });
	else if (row.kind === "zoom") store.updateZoomRegion(id, { scale: v });
	else if (row.kind === "markup") store.updateAnnotation(id, { opacity: v });
}

function togglePlay() {
	if (!videoEl) return;
	if (store.isPlaying) {
		videoEl.pause();
		store.isPlaying = false;
	} else {
		void videoEl.play();
		store.isPlaying = true;
	}
}
function splitAtPlayhead() {
	store.splitAt(store.currentTime);
}
function clearSelection() {
	store.selectedClipStart = null;
	store.selectedZoomRegionId = null;
	store.selectedAnnotationId = null;
	store.selectedMusicClipId = null;
}

// Add a 2s zoom region at the playhead (original axis, where currentTime lives).
function addZoomAtPlayhead() {
	const s = store.currentTime;
	const end = Math.min(store.outPoint, s + 2);
	if (end > s) store.addZoomRegion(s, end);
}

// Delete whatever is selected: zoom / markup / audio clip, or ripple-delete the
// selected video segment. The store methods push their own undo entry.
function deleteSelected() {
	if (store.selectedZoomRegionId) store.removeZoomRegion(store.selectedZoomRegionId);
	else if (store.selectedAnnotationId) store.removeAnnotation(store.selectedAnnotationId);
	else if (store.selectedMusicClipId) store.removeMusicClip(store.selectedMusicClipId);
	else if (store.selectedClipStart !== null) store.deleteSegmentAt(store.selectedClipStart);
}

function zoomToFit() {
	if (totalFrames <= 0) return;
	view = clampScroll(
		zoomAt(
			{
				scrollFrames: 0,
				resolution: (contentW * 0.98) / totalFrames,
			},
			0,
			1,
		),
		totalFrames,
		contentW,
	);
}
function zoomStep(factor: number) {
	userAdjusted = true;
	view = clampScroll(zoomAt(view, contentW / 2, factor), totalFrames, contentW);
}

// Step one frame on the OUTPUT axis (lands on the next kept frame across a cut).
function stepFrame(dir: number) {
	if (!store.metadata) return;
	store.seek(frameStepOutput(store.timeMap, store.metadata, store.currentTime, dir));
}
// Mark in/out set the recording trim at the playhead; keep a forward window.
function markIn() {
	if (store.currentTime < store.outPoint - 0.05) store.trimStart = store.currentTime;
}
function markOut() {
	if (store.currentTime > store.inPoint + 0.05) store.trimEnd = store.currentTime;
}
function resetTrim() {
	store.trimStart = 0;
	store.trimEnd = store.metadata?.duration ?? 0;
}

const SPEED_PRESETS = [0.5, 1, 1.5, 2] as const;

// One storyboard sprite (cols×rows frame cells) is enough for a filmstrip —
// far cheaper than per-tile decode. Loaded once; a new url reloads it.
let sbUrl: string | null = null;
let sbImg: HTMLImageElement | null = null;
let sbMeta: Storyboard | null = null;
function ensureStoryboard(): boolean {
	const sb = tileProvider?.storyboard();
	if (!sb) return false;
	if (sb.url !== sbUrl) {
		sbUrl = sb.url;
		sbMeta = sb;
		const img = new Image();
		img.onload = () => {
			sbImg = img;
			scheduleDraw();
		};
		img.src = sb.url;
	}
	return sbImg !== null && sbMeta !== null;
}

type ClipRect = { x: number; w: number; y: number; h: number };

// --- colours: the canvas can't read CSS vars, so resolve --tl-* once (and on
// theme flips) to concrete strings via a probe span. ---
type ClipPaint = { bg: string; primary: string; on: string };
let tl = $state({
	surface: "#1c1c1c",
	surfaceMuted: "#292929",
	border: "#1c1c1c",
	borderInput: "#3e3f41",
	ring: "#008cff",
	scrubber: "#f43535",
	rulerTick: "rgba(53,53,53,1)",
	rulerText: "rgba(120,120,120,1)",
	label: "#d4d4d8",
});
let clipPaint = $state<Record<ClipKind, ClipPaint>>({
	video: { bg: "#0f3c8a", primary: "#70a7ff", on: "#cce0ff" },
	zoom: { bg: "#066284", primary: "#5ab9dd", on: "#dbeaf0" },
	markup: { bg: "#933325", primary: "#d98a7a", on: "#faedeb" },
	caption: { bg: "#7b1e5a", primary: "#aa317b", on: "#f9dced" },
	audio: { bg: "#004732", primary: "#0dbf8a", on: "#cbfaed" },
});

function readColors() {
	if (!containerEl) return;
	const probe = document.createElement("span");
	probe.style.cssText = "position:absolute;width:0;height:0;visibility:hidden";
	containerEl.appendChild(probe);
	const read = (token: string, fallback: string) => {
		probe.style.color = `var(${token})`;
		const c = getComputedStyle(probe).color;
		return c && c !== "" ? c : fallback;
	};
	// Chrome/surfaces use the app tokens so the timeline matches the panels and
	// tracks the theme; only the clip bodies keep the categorical --tl-* palette.
	tl = {
		surface: read("--background", "#1c1c1c"),
		surfaceMuted: read("--muted", "#292929"),
		border: read("--border", "#2a2a2a"),
		borderInput: read("--border", "#3e3f41"),
		ring: read("--primary", "#008cff"),
		scrubber: read("--destructive", "#f43535"),
		rulerTick: read("--border", "rgba(53,53,53,1)"),
		rulerText: read("--muted-foreground", "rgba(120,120,120,1)"),
		label: read("--foreground", "#d4d4d8"),
	};
	const paint = (k: ClipKind): ClipPaint => ({
		bg: read(`--tl-${k}-bg`, clipPaint[k].bg),
		primary: read(`--tl-${k}-primary`, clipPaint[k].primary),
		on: read(`--tl-${k}-on`, clipPaint[k].on),
	});
	clipPaint = {
		video: paint("video"),
		zoom: paint("zoom"),
		markup: paint("markup"),
		caption: paint("caption"),
		audio: paint("audio"),
	};
	probe.remove();
	scheduleDraw();
}

function sizeCanvas() {
	if (!containerEl || !canvasEl) return;
	const rect = canvasEl.getBoundingClientRect();
	cssW = Math.max(1, Math.round(rect.width));
	cssH = Math.max(1, Math.round(rect.height));
	dpr = Math.max(1, window.devicePixelRatio || 1);
	canvasEl.width = Math.round(cssW * dpr);
	canvasEl.height = Math.round(cssH * dpr);
	canvasEl.style.width = `${cssW}px`;
	canvasEl.style.height = `${cssH}px`;
	// Refit on every resize until the user zooms/pans: a one-shot fit locked the
	// stale first-mount width, so the clip under-filled and the pointer mismapped.
	if (totalFrames > 0 && contentW > 40 && !userAdjusted) {
		view = clampScroll(
			{
				scrollFrames: 0,
				resolution: (contentW * 0.98) / totalFrames,
			},
			totalFrames,
			contentW,
		);
	}
	scrollY = clampScrollY(scrollY);
	scheduleDraw();
}

function maxScrollY(): number {
	return Math.max(0, contentHeight - (cssH - RULER_HEIGHT));
}
function clampScrollY(v: number): number {
	return Math.min(Math.max(0, v), maxScrollY());
}

/** Canvas y of a row's clip band, or null when the row is scrolled out. */
function rowClipTop(laid: LaidRow): number {
	return RULER_HEIGHT + laid.top - scrollY;
}

let rafId = 0;
function scheduleDraw() {
	if (rafId) return; // one frame in flight; drop late frames rather than queue
	rafId = requestAnimationFrame(() => {
		rafId = 0;
		draw();
	});
}

function draw() {
	if (!ctx) return;
	ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
	ctx.fillStyle = tl.surface;
	ctx.fillRect(0, 0, cssW, cssH);

	// Time content is drawn to the right of the gutter (the header overlays it).
	ctx.translate(GUTTER, 0);

	ctx.save();
	ctx.beginPath();
	ctx.rect(0, RULER_HEIGHT, contentW, cssH - RULER_HEIGHT);
	ctx.clip();
	drawRows();
	drawCutSeams();
	ctx.restore();

	drawRuler();
	drawSnapGuide();
	drawPlayhead();
}

function drawRows() {
	if (!ctx) return;
	if (rowLayout.length === 0) {
		ctx.fillStyle = tl.rulerText;
		ctx.font = "11px Inter, system-ui, sans-serif";
		ctx.textBaseline = "middle";
		ctx.fillText("No clips yet", 12, RULER_HEIGHT + ROW_H);
		return;
	}
	for (let i = 0; i < rowLayout.length; i++) {
		const laid = rowLayout[i];
		const top = rowClipTop(laid);
		const groupH = laid.height + (laid.expanded ? SUBROW_H : 0);
		if (top + groupH < RULER_HEIGHT || top > cssH) continue;
		// Row band.
		ctx.fillStyle = tl.surfaceMuted;
		ctx.globalAlpha = i % 2 === 0 ? 0.5 : 0.32;
		roundRectPath(0, top, contentW, laid.height, CLIP_RADIUS);
		ctx.fill();
		if (laid.expanded) {
			// Property track band, a touch dimmer than the clip band.
			ctx.globalAlpha = 0.22;
			roundRectPath(0, top + laid.height, contentW, SUBROW_H, CLIP_RADIUS);
			ctx.fill();
		}
		ctx.globalAlpha = 1;
		for (const clip of laid.row.clips) drawClip(clip, top, laid.height);
	}
}

/** Align a CSS-px value to a whole device pixel so edges stay crisp under DPR. */
function snapDev(v: number): number {
	return Math.round(v * dpr) / dpr;
}

// Silence-cut seams: a small neutral notch at the top of the lanes, never a
// full-height coloured line (that reads as the playhead).
function drawCutSeams() {
	if (!ctx || cutFrames.length === 0) return;
	for (const f of cutFrames) {
		const x = Math.round(frameToX(f, view)) + 0.5;
		if (x < -2 || x > contentW + 2) continue;
		ctx.fillStyle = tl.rulerText;
		ctx.globalAlpha = 0.9;
		ctx.beginPath();
		ctx.moveTo(x - 3, RULER_HEIGHT);
		ctx.lineTo(x + 3, RULER_HEIGHT);
		ctx.lineTo(x, RULER_HEIGHT + 4);
		ctx.closePath();
		ctx.fill();
		ctx.strokeStyle = tl.rulerText;
		ctx.globalAlpha = 0.18;
		ctx.lineWidth = 1;
		ctx.beginPath();
		ctx.moveTo(x, RULER_HEIGHT);
		ctx.lineTo(x, cssH);
		ctx.stroke();
	}
	ctx.globalAlpha = 1;
}

function roundRectPath(x: number, y: number, w: number, h: number, r: number) {
	if (!ctx) return;
	ctx.beginPath();
	ctx.roundRect(x, y, w, h, Math.min(r, w / 2, h / 2));
}

function truncate(text: string, maxPx: number): string {
	if (!ctx || ctx.measureText(text).width <= maxPx) return text;
	let t = text;
	while (t.length > 1 && ctx.measureText(`${t}…`).width > maxPx) t = t.slice(0, -1);
	return `${t}…`;
}

function drawClip(clip: TimelineClip, top: number, h: number) {
	if (!ctx) return;
	const x = snapDev(frameToX(clip.start, view));
	const w = Math.max(2, snapDev(clip.duration * view.resolution));
	if (x + w < 0 || x > contentW) return;
	const paint = clipPaint[clip.kind];
	const r: ClipRect = { x, w, y: top, h };

	ctx.globalAlpha = clip.hidden ? 0.5 : 1;
	ctx.fillStyle = paint.bg;
	roundRectPath(x, top, w, h, CLIP_RADIUS);
	ctx.fill();

	drawClipContent(clip, r, paint);
	if (w >= 24) drawClipLabel(clip, r, paint);
	drawClipDecorations(clip, r, paint);
	ctx.globalAlpha = 1;
}

// Video: picture strip on top, waveform in its own band below (never overlapped).
function drawClipContent(clip: TimelineClip, r: ClipRect, paint: ClipPaint) {
	const wave = hasWaveform();
	if (clip.kind === "video") {
		const splitWave = wave && r.h >= VIDEO_WAVE_MIN_H;
		const picH = splitWave ? r.h - WAVE_BAND : r.h;
		drawFilmstrip(clip, r.x, r.w, r.y, picH);
		if (splitWave)
			drawWaveformBars({ x: r.x, w: r.w, y: r.y + picH, h: WAVE_BAND }, paint.primary, 0.85);
	} else if (clip.kind === "audio" && wave) {
		drawWaveformBars(r, paint.primary, 1);
	}
}

// A translucent chip behind the label so it stays legible over the picture strip.
function drawClipLabel(clip: TimelineClip, r: ClipRect, paint: ClipPaint) {
	if (!ctx) return;
	ctx.save();
	roundRectPath(r.x, r.y, r.w, r.h, CLIP_RADIUS);
	ctx.clip();
	const label = truncate(clip.label, r.w - 2 * CLIP_LABEL_X);
	const tw = ctx.measureText(label).width;
	ctx.fillStyle = tl.surface;
	ctx.globalAlpha = clip.hidden ? 0.3 : 0.55;
	roundRectPath(r.x + 3, r.y + 3, tw + 8, CLIP_LABEL_HEIGHT, 3);
	ctx.fill();
	ctx.globalAlpha = clip.hidden ? 0.5 : 1;
	ctx.fillStyle = paint.on;
	ctx.font = "11px Inter, system-ui, sans-serif";
	ctx.textBaseline = "middle";
	ctx.textAlign = "left";
	ctx.fillText(label, r.x + CLIP_LABEL_X + 1, r.y + 3 + CLIP_LABEL_HEIGHT / 2);
	ctx.restore();
}

// Trim grips (on a selected editable clip) and the border/selection ring.
function drawClipDecorations(clip: TimelineClip, r: ClipRect, paint: ClipPaint) {
	if (!ctx) return;
	if (clip.selected && canTrim(clip) && r.w > TRIM_HANDLE_PX * 2) {
		ctx.fillStyle = paint.on;
		ctx.globalAlpha = 0.55;
		ctx.fillRect(r.x + 2, r.y + r.h / 2 - 6, 1.5, 12);
		ctx.fillRect(r.x + r.w - 3.5, r.y + r.h / 2 - 6, 1.5, 12);
		ctx.globalAlpha = clip.hidden ? 0.5 : 1;
	}
	ctx.save();
	roundRectPath(r.x, r.y, r.w, r.h, CLIP_RADIUS);
	ctx.clip();
	ctx.strokeStyle = clip.selected ? tl.ring : tl.border;
	ctx.lineWidth = clip.selected ? 2 : 1;
	roundRectPath(r.x, r.y, r.w, r.h, CLIP_RADIUS);
	ctx.stroke();
	ctx.restore();
}

function hasWaveform(): boolean {
	return (store.waveform?.length ?? 0) >= 2 && (store.metadata?.duration ?? 0) > 0;
}

// object-fit: cover, from the whole image.
function drawCover(img: HTMLImageElement, dx: number, dy: number, dw: number, dh: number) {
	if (!ctx) return;
	const iw = img.naturalWidth;
	const ih = img.naturalHeight;
	if (!iw || !ih) return;
	const scale = Math.max(dw / iw, dh / ih);
	const sw = dw / scale;
	const sh = dh / scale;
	ctx.drawImage(img, (iw - sw) / 2, (ih - sh) / 2, sw, sh, dx, dy, dw, dh);
}

// object-fit: cover, from a sub-rect `s` of a sprite into `d`.
function drawCoverCrop(img: HTMLImageElement, s: ClipRect, d: ClipRect) {
	if (!ctx) return;
	const scale = Math.max(d.w / s.w, d.h / s.h);
	const cw = d.w / scale;
	const ch = d.h / scale;
	ctx.drawImage(img, s.x + (s.w - cw) / 2, s.y + (s.h - ch) / 2, cw, ch, d.x, d.y, d.w, d.h);
}

// Decoded tiles are object URLs owned by the provider; we only cache the Image.
const tileImgs = new Map<string, HTMLImageElement>();
function ensureTileImage(url: string): HTMLImageElement | null {
	const cached = tileImgs.get(url);
	if (cached) return cached.complete && cached.naturalWidth > 0 ? cached : null;
	const img = new Image();
	img.onload = () => scheduleDraw();
	img.src = url;
	tileImgs.set(url, img);
	if (tileImgs.size > 320) {
		const oldest = tileImgs.keys().next().value;
		if (oldest) tileImgs.delete(oldest);
	}
	return null;
}

// Virtualized filmstrip: plan on-screen tiles, request their decode, draw each
// decoded frame; a tile still decoding falls back to the coarse storyboard cell,
// then to a muted fill — never solid black.
function drawFilmstrip(clip: TimelineClip, x0: number, cw: number, y: number, h: number): void {
	if (!ctx || h <= 0) return;
	const seg = store.segments.find((s) => String(s.start) === clip.id);
	if (!seg) return;
	const outStartFrame = originalToOutput(store.renderMap, seg.start) * fps;
	const block: FilmstripBlock = {
		key: seg.start,
		leftPx: outStartFrame * view.resolution,
		widthPx: cw,
		originalStart: seg.start,
		originalEnd: seg.end,
	};
	const tiles = planFilmstrip(
		[block],
		{ leftPx: view.scrollFrames * view.resolution, widthPx: contentW },
		{
			tileWidthPx: FILMSTRIP_TILE_WIDTH,
			tileHeightPx: Math.round(FILMSTRIP_TILE_HEIGHT * dpr),
			overscanPx: 240,
		},
	);
	tileProvider?.request(tiles);

	ctx.save();
	roundRectPath(x0, y, cw, h, CLIP_RADIUS);
	ctx.clip();
	ctx.fillStyle = tl.surfaceMuted;
	ctx.globalAlpha = 0.6;
	ctx.fillRect(x0, y, cw, h);
	ctx.globalAlpha = 1;
	for (const t of tiles) {
		const tx = snapDev(x0 + t.offsetPx);
		const tw = Math.ceil(t.widthPx) + 1;
		const url = tileProvider?.get(t);
		const img = url ? ensureTileImage(url) : null;
		if (img) drawCover(img, tx, y, tw, h);
		else drawStoryboardCell(tx, y, tw, h, t.sampleOriginalSec);
	}
	ctx.restore();
}

function drawStoryboardCell(tx: number, y: number, tw: number, h: number, origSec: number): void {
	if (!ctx || !ensureStoryboard() || !sbImg || !sbMeta) return;
	const meta = sbMeta;
	const i = Math.max(
		0,
		Math.min(meta.count - 1, Math.floor((origSec / Math.max(meta.durationSec, 1e-6)) * meta.count)),
	);
	drawCoverCrop(
		sbImg,
		{
			x: (i % meta.cols) * meta.cellW,
			y: Math.floor(i / meta.cols) * meta.cellH,
			w: meta.cellW,
			h: meta.cellH,
		},
		{ x: tx, y, w: tw, h },
	);
}

// Recording waveform centred in its band. `alpha` dims it over the video strip.
function drawWaveformBars(r: ClipRect, color: string, alpha: number): void {
	if (!ctx) return;
	const wf = store.waveform;
	const dur = store.metadata?.duration ?? 0;
	if (!wf || wf.length < 2 || dur <= 0) return;

	ctx.save();
	roundRectPath(r.x, r.y, r.w, r.h, CLIP_RADIUS);
	ctx.clip();
	ctx.fillStyle = color;
	ctx.globalAlpha = alpha;

	const bandH = Math.max(2, r.h - 3);
	const inSec = store.inPoint;
	const outSec = store.outPoint;

	for (let sx = 0; sx < r.w; sx += SAMPLE_WIDTH) {
		const oSec = xToFrame(r.x + sx, view) / fps;
		const origSec = outputToOriginal(store.renderMap, oSec);
		if (origSec < inSec - 0.01 || origSec > outSec + 0.01) continue;
		const i = Math.max(0, Math.min(wf.length - 1, Math.floor((origSec / dur) * wf.length)));
		const bh = Math.max((wf[i] ?? 0) * bandH, 1);
		ctx.fillRect(snapDev(r.x + sx), r.y + (r.h - bh) / 2, SAMPLE_WIDTH - 0.4, bh);
	}
	ctx.globalAlpha = 1;
	ctx.restore();
}

function fmtTick(frame: number): string {
	const f = fps > 0 ? fps : 60;
	if (frame <= 0) return "0";
	if (frame % f !== 0) return `${Math.round(frame)}f`;
	const total = Math.round(frame / f);
	const m = Math.floor(total / 60)
		.toString()
		.padStart(2, "0");
	const s = Math.floor(total % 60)
		.toString()
		.padStart(2, "0");
	return `${m}:${s}`;
}

function drawRuler() {
	if (!ctx) return;
	ctx.fillStyle = tl.surface;
	ctx.fillRect(0, 0, contentW, RULER_HEIGHT);

	const { ticks } = visibleTicks(view, contentW, fps, totalFrames);
	ctx.font = '300 10px "JetBrains Mono", ui-monospace, monospace';
	ctx.textBaseline = "middle";
	ctx.textAlign = "center";
	for (const t of ticks) {
		const x = Math.round(t.x) + 0.5;
		ctx.strokeStyle = tl.rulerTick;
		ctx.lineWidth = 1;
		ctx.beginPath();
		ctx.moveTo(x, RULER_HEIGHT - (t.major ? RULER_TICK_MAJOR : RULER_TICK_MINOR));
		ctx.lineTo(x, RULER_HEIGHT);
		ctx.stroke();
		if (t.major) {
			ctx.fillStyle = tl.rulerText;
			ctx.fillText(fmtTick(t.frame), Math.round(t.x), RULER_LABEL_Y);
		}
	}
}

function drawSnapGuide() {
	if (!ctx || snapGuideFrame === null) return;
	const x = Math.round(frameToX(snapGuideFrame, view)) + 0.5;
	ctx.strokeStyle = tl.ring;
	ctx.globalAlpha = 0.8;
	ctx.lineWidth = 1;
	ctx.setLineDash([3, 3]);
	ctx.beginPath();
	ctx.moveTo(x, RULER_HEIGHT);
	ctx.lineTo(x, cssH);
	ctx.stroke();
	ctx.setLineDash([]);
	ctx.globalAlpha = 1;
}

// The playhead: a shield knob in the ruler, a blue line with a dark halo, and a
// velocity-driven motion trail so a scrub eases rather than snaps.
const GRADIENT_PX_PER_VELOCITY = 25;
const GRADIENT_RESPONSE_TIME = 0.15;
const GRADIENT_MAX_WIDTH = 150;
let gradientWidth = 0;
let gradientVelocity = 0;
let lastPlayFrame = 0;
let lastPlayTs = 0;

function drawPlayhead() {
	if (!ctx) return;
	// Snap to a whole device pixel like the ticks/clips: an unsnapped 1px line
	// anti-aliases across two columns and washes out over the dark lanes.
	const x = snapDev(frameToX(playheadFrame, view));

	ctx.save();
	ctx.translate(x, 0);
	drawMotionTrail();

	// The knob, sitting in the ruler — Diffusion's exact KNOB_PATH (a 10px pin,
	// centred with translate(-5) and dropped 2px), dark outline over blue fill.
	ctx.save();
	ctx.translate(-5, 2);
	ctx.strokeStyle = tl.border;
	ctx.lineWidth = 2;
	ctx.stroke(KNOB_PATH);
	ctx.fillStyle = tl.ring;
	ctx.fill(KNOB_PATH);
	ctx.restore();

	// The line below the ruler: a dark halo (4px) under the blue scrubber (2px),
	// both crisp because x is device-snapped.
	ctx.beginPath();
	ctx.moveTo(0, RULER_HEIGHT);
	ctx.lineTo(0, cssH);
	ctx.strokeStyle = tl.border;
	ctx.lineWidth = 4;
	ctx.stroke();
	ctx.strokeStyle = tl.ring;
	ctx.lineWidth = 2;
	ctx.stroke();
	ctx.restore();
}

function drawMotionTrail() {
	if (!ctx) return;
	const now = performance.now();
	const elapsed = lastPlayTs ? (now - lastPlayTs) / 1000 : 0;
	lastPlayTs = now;
	const velocity = elapsed > 0 ? (playheadFrame - lastPlayFrame) / Math.max(fps, 1) / elapsed : 0;
	lastPlayFrame = playheadFrame;

	const target = Math.max(
		-GRADIENT_MAX_WIDTH,
		Math.min(GRADIENT_MAX_WIDTH, GRADIENT_PX_PER_VELOCITY * velocity),
	);
	if (elapsed > 0) {
		const omega = 2 / GRADIENT_RESPONSE_TIME;
		const dt = Math.min(elapsed, 0.25);
		const decay = Math.exp(-omega * dt);
		const error = gradientWidth - target;
		const b = gradientVelocity + omega * error;
		gradientWidth = target + (error + b * dt) * decay;
		gradientVelocity = (gradientVelocity - omega * b * dt) * decay;
	}
	const width = Math.abs(gradientWidth);
	if (width <= 0.5) return;
	const reverse = gradientWidth < 0;
	const grad = ctx.createLinearGradient(reverse ? width : -width, 0, 0, 0);
	// `transparent`, not `${tl.ring}00`: the hex-alpha suffix is invalid on the
	// oklch()/rgb() strings getComputedStyle returns and throws in addColorStop.
	grad.addColorStop(0, "transparent");
	grad.addColorStop(1, tl.ring);
	ctx.fillStyle = grad;
	ctx.globalAlpha = 0.16;
	ctx.fillRect(reverse ? 0 : -width, RULER_HEIGHT, width, cssH - RULER_HEIGHT);
	ctx.globalAlpha = 1;
}

// --- interaction ---

// Content x: pointer position relative to the time area (past the header gutter).
function localX(e: PointerEvent | WheelEvent): number {
	const rect = canvasEl?.getBoundingClientRect();
	return rect ? e.clientX - rect.left - GUTTER : 0;
}
function localY(e: PointerEvent): number {
	const rect = canvasEl?.getBoundingClientRect();
	return rect ? e.clientY - rect.top : 0;
}

function seekToX(x: number) {
	if (fps <= 0) return;
	const frame = Math.min(Math.max(xToFrame(x, view), 0), totalFrames);
	store.seek(outputToOriginal(store.renderMap, frame / fps));
}

type Gesture =
	| { kind: "scrub" }
	| {
			kind: "move";
			clip: TimelineClip;
			grabFrame: number;
			startFrame: number;
			durFrames: number;
	  }
	| {
			kind: "trim";
			edge: "l" | "r";
			clip: TimelineClip;
			startFrame: number;
			endFrame: number;
	  };
let gesture: Gesture | null = null;
let snapGuideFrame: number | null = null;

function canMove(clip: TimelineClip): boolean {
	return clip.kind === "zoom" || clip.kind === "markup" || clip.kind === "audio";
}
function canTrim(clip: TimelineClip): boolean {
	return canMove(clip) || (clip.kind === "video" && videoTrimmable);
}

function clipAt(x: number, y: number): TimelineClip | null {
	for (const laid of rowLayout) {
		const top = rowClipTop(laid);
		if (y < top || y >= top + laid.height) continue;
		for (let i = laid.row.clips.length - 1; i >= 0; i--) {
			const clip = laid.row.clips[i];
			const cx = frameToX(clip.start, view);
			const cw = Math.max(2, clip.duration * view.resolution);
			if (x >= cx && x < cx + cw) return clip;
		}
	}
	return null;
}

function edgeZone(clip: TimelineClip, x: number): "l" | "r" | "body" {
	const cx = frameToX(clip.start, view);
	const cw = Math.max(2, clip.duration * view.resolution);
	if (cw <= TRIM_HANDLE_PX * 2) return "body";
	if (x - cx <= TRIM_HANDLE_PX) return "l";
	if (cx + cw - x <= TRIM_HANDLE_PX) return "r";
	return "body";
}

function selectClip(clip: TimelineClip) {
	switch (clip.kind) {
		case "video":
			store.selectedClipStart = Number(clip.id);
			break;
		case "zoom":
			store.selectedZoomRegionId = clip.id;
			break;
		case "markup":
			store.selectedAnnotationId = clip.id;
			break;
		case "audio":
			store.selectedMusicClipId = clip.id;
			break;
		default:
			break;
	}
}

function snapFrame(frame: number, exclude: TimelineClip): number {
	const dist = SNAP_PX / Math.max(view.resolution, 1e-6);
	const targets = [0, totalFrames, playheadFrame];
	for (const c of allClips) {
		if (c.kind === exclude.kind && c.id === exclude.id) continue;
		targets.push(c.start, c.start + c.duration);
	}
	let best = frame;
	let bestDist = dist;
	let snapped = false;
	for (const t of targets) {
		const d = Math.abs(frame - t);
		if (d < bestDist) {
			bestDist = d;
			best = t;
			snapped = true;
		}
	}
	snapGuideFrame = snapped ? best : null;
	return best;
}

function applyClipSpan(clip: TimelineClip, startFrame: number, endFrame: number) {
	const startSec = startFrame / fps;
	const endSec = endFrame / fps;
	switch (clip.kind) {
		case "zoom":
			store.updateZoomRegion(clip.id, {
				start: outputToOriginal(store.renderMap, startSec),
				end: outputToOriginal(store.renderMap, endSec),
			});
			break;
		case "markup":
			store.updateAnnotation(clip.id, {
				start: outputToOriginal(store.renderMap, startSec),
				end: outputToOriginal(store.renderMap, endSec),
			});
			break;
		case "audio":
			store.updateMusicClip(clip.id, {
				startOutputSec: startSec,
				durationSec: Math.max(0, endSec - startSec),
			});
			break;
		case "video":
			store.trimStart = outputToOriginal(store.renderMap, startSec);
			store.trimEnd = outputToOriginal(store.renderMap, endSec);
			break;
		default:
			break;
	}
}

function hoverCursor(x: number, y: number): string {
	if (y < RULER_HEIGHT || nearPlayhead(x)) return "ew-resize";
	const clip = clipAt(x, y);
	if (!clip) return "default";
	if (clip.kind === "video" && !videoTrimmable) return "pointer";
	if (!canMove(clip) && !canTrim(clip)) return "pointer";
	const zone = edgeZone(clip, x);
	if (zone !== "body") return "ew-resize";
	return canMove(clip) ? "grab" : "pointer";
}

// True when x is over the playhead line, so a press on the lanes grabs the
// scrubber rather than the clip beneath it.
function nearPlayhead(x: number): boolean {
	return Math.abs(frameToX(playheadFrame, view) - x) <= PLAYHEAD_GRAB_PX;
}
function startScrub(x: number, e: PointerEvent) {
	if (store.isPlaying) {
		videoEl?.pause();
		store.isPlaying = false;
	}
	gesture = { kind: "scrub" };
	canvasEl?.setPointerCapture(e.pointerId);
	seekToX(x);
}

function onPointerDown(e: PointerEvent) {
	if (e.button !== 0) return;
	const x = localX(e);
	const y = localY(e);
	// The ruler scrubs to the point; the line itself is grabbable over the lanes.
	if (y < RULER_HEIGHT || nearPlayhead(x)) {
		startScrub(x, e);
		return;
	}
	const clip = clipAt(x, y);
	if (clip) {
		selectClip(clip);
		const zone = edgeZone(clip, x);
		const startTrim = zone !== "body" && canTrim(clip);
		const startMove = zone === "body" && canMove(clip);
		if (startTrim || startMove) {
			store.pushUndoState();
			gesture = startMove
				? {
						kind: "move",
						clip,
						grabFrame: xToFrame(x, view),
						startFrame: clip.start,
						durFrames: clip.duration,
					}
				: {
						kind: "trim",
						edge: zone as "l" | "r",
						clip,
						startFrame: clip.start,
						endFrame: clip.start + clip.duration,
					};
			canvasEl?.setPointerCapture(e.pointerId);
		}
		return;
	}
	clearSelection();
}

function onPointerMove(e: PointerEvent) {
	const x = localX(e);
	if (!gesture) {
		if (canvasEl) canvasEl.style.cursor = hoverCursor(x, localY(e));
		return;
	}
	if (gesture.kind === "scrub") {
		seekToX(x);
		return;
	}
	const frame = xToFrame(x, view);
	if (gesture.kind === "move") {
		const start = Math.max(
			0,
			snapFrame(gesture.startFrame + (frame - gesture.grabFrame), gesture.clip),
		);
		applyClipSpan(gesture.clip, start, start + gesture.durFrames);
	} else if (gesture.edge === "l") {
		const start = Math.max(
			0,
			snapFrame(Math.min(frame, gesture.endFrame - MIN_CLIP_FRAMES), gesture.clip),
		);
		applyClipSpan(gesture.clip, start, gesture.endFrame);
	} else {
		const end = snapFrame(Math.max(frame, gesture.startFrame + MIN_CLIP_FRAMES), gesture.clip);
		applyClipSpan(gesture.clip, gesture.startFrame, end);
	}
	scheduleDraw();
}

function onPointerUp(e: PointerEvent) {
	gesture = null;
	snapGuideFrame = null;
	canvasEl?.releasePointerCapture(e.pointerId);
	scheduleDraw();
}
function onPointerLeave() {
	if (!gesture && canvasEl) canvasEl.style.cursor = "default";
	scheduleDraw();
}

// --- context menu (replaces the native right-click menu) ---

let menuClip = $state<TimelineClip | null>(null);

function onContextMenu(e: MouseEvent) {
	const rect = canvasEl?.getBoundingClientRect();
	menuClip = rect ? clipAt(e.clientX - rect.left - GUTTER, e.clientY - rect.top) : null;
	if (menuClip) selectClip(menuClip);
}
function menuDelete() {
	const c = menuClip;
	if (!c) return;
	if (c.kind === "video") {
		store.deleteSegmentAt(Number(c.id));
		return;
	}
	store.pushUndoState();
	if (c.kind === "zoom") store.removeZoomRegion(c.id);
	else if (c.kind === "markup") store.removeAnnotation(c.id);
	else if (c.kind === "audio") store.removeMusicClip(c.id);
}
function menuDuplicate() {
	const c = menuClip;
	if (c?.kind === "zoom") store.duplicateZoomRegion(c.id);
	else if (c?.kind === "markup") store.duplicateAnnotation(c.id);
}
function menuToggleHidden() {
	const c = menuClip;
	if (c?.kind === "zoom") store.setZoomRegionHidden(c.id);
	else if (c?.kind === "markup") store.toggleAnnotationVisibility(c.id);
}
function menuToggleLock() {
	if (menuClip?.kind === "markup") store.toggleAnnotationLock(menuClip.id);
}
function menuSetSpeed(v: number) {
	if (menuClip?.kind === "video") store.setSegmentSpeed(Number(menuClip.id), v);
}
const menuSpeed = $derived(
	menuClip?.kind === "video" ? store.segmentSpeedAt(Number(menuClip.id)) : 1,
);
const menuCanEdit = $derived(menuClip?.kind === "zoom" || menuClip?.kind === "markup");
const menuCanDelete = $derived(menuClip != null && menuClip.kind !== "caption");

// Normalize line/page wheel deltas so a physical mouse wheel pans/zooms the same
// as a trackpad (Diffusion's normalizeWheel: LINE≈16px, PAGE≈600px).
function normDelta(d: number, mode: number): number {
	if (mode === 1) return d * 16;
	if (mode === 2) return d * 600;
	return d;
}
function onWheel(e: WheelEvent) {
	e.preventDefault();
	const dY = normDelta(e.deltaY, e.deltaMode);
	const dX = normDelta(e.deltaX, e.deltaMode);
	if (e.ctrlKey || e.metaKey) {
		userAdjusted = true;
		const clamped = Math.max(-ZOOM_DELTA_CLAMP, Math.min(ZOOM_DELTA_CLAMP, dY));
		view = clampScroll(
			zoomAt(view, localX(e), Math.exp(-clamped * ZOOM_SENSITIVITY)),
			totalFrames,
			contentW,
		);
		return;
	}
	const dxDominant = Math.abs(dX) > Math.abs(dY);
	if (!e.shiftKey && !dxDominant && maxScrollY() > 0) {
		scrollY = clampScrollY(scrollY + dY);
		return;
	}
	userAdjusted = true;
	const dx = (dxDominant ? dX : dY) * SCROLL_X_SENSITIVITY;
	view = clampScroll(scrollByPixels(view, dx), totalFrames, contentW);
}

function onKeyDown(e: KeyboardEvent) {
	// Transport keys belong to media-controller; the timeline owns editing keys.
	if (e.metaKey || e.ctrlKey || e.altKey) return;
	switch (e.key) {
		case "Home":
			e.preventDefault();
			store.seek(0);
			break;
		case "End":
			e.preventDefault();
			store.seek(outputToOriginal(store.renderMap, outputDurationSec));
			break;
		case "Delete":
		case "Backspace":
			e.preventDefault();
			deleteSelected();
			break;
		case "s":
		case "S":
			e.preventDefault();
			splitAtPlayhead();
			break;
		case "z":
		case "Z":
			e.preventDefault();
			addZoomAtPlayhead();
			break;
		case "f":
		case "F":
			e.preventDefault();
			zoomToFit();
			break;
		case "+":
		case "=":
			e.preventDefault();
			zoomStep(1.25);
			break;
		case "-":
		case "_":
			e.preventDefault();
			zoomStep(0.8);
			break;
		case "ArrowLeft":
			e.preventDefault();
			stepFrame(-1);
			break;
		case "ArrowRight":
			e.preventDefault();
			stepFrame(1);
			break;
		case "[":
			e.preventDefault();
			markIn();
			break;
		case "]":
			e.preventDefault();
			markOut();
			break;
		case "k":
		case "K":
			e.preventDefault();
			togglePlay();
			break;
		default:
			break;
	}
}

// Time readout respects the editor's Time-display mode (SMPTE / seconds / frames).
function timecode(frame: number): string {
	return formatTimeByMode(frame / Math.max(fps, 1), store.timeMode, fps);
}

// The header column scrolls its rows with the canvas.
const headerScrollStyle = $derived(`transform:translateY(${-scrollY}px);padding-top:${ROW_GAP}px`);

onMount(() => {
	ctx = canvasEl?.getContext("2d", { alpha: false }) ?? null;
	expandedRows = loadExpanded();
	readColors();
	sizeCanvas();

	const ro = new ResizeObserver(sizeCanvas);
	if (canvasEl) ro.observe(canvasEl);

	const themeObserver = new MutationObserver(readColors);
	themeObserver.observe(document.documentElement, {
		attributes: true,
		attributeFilter: ["class", "data-theme", "style"],
	});
	const scheme = window.matchMedia("(prefers-color-scheme: dark)");
	scheme.addEventListener("change", readColors);

	return () => {
		ro.disconnect();
		themeObserver.disconnect();
		scheme.removeEventListener("change", readColors);
		if (rafId) cancelAnimationFrame(rafId);
		tileImgs.clear();
	};
});

// The filmstrip decoder shares hardware with the preview; pause it while playing.
$effect(() => {
	tileProvider?.setDecodePaused(store.isPlaying);
});

// Keep the playhead in view while playing (auto-scroll like an NLE). Reads the
// view untracked so writing scrollFrames doesn't re-trigger the effect.
$effect(() => {
	if (!store.isPlaying) return;
	const pf = playheadFrame;
	untrack(() => {
		if (view.resolution <= 0) return;
		const spanF = contentW / view.resolution;
		const leftF = view.scrollFrames;
		if (pf < leftF || pf > leftF + spanF - 4) {
			view = clampScroll(
				{ ...view, scrollFrames: Math.max(0, pf - spanF * 0.1) },
				totalFrames,
				contentW,
			);
		}
	});
});

function redrawOn(..._deps: unknown[]): void {
	scheduleDraw();
}
$effect(() => {
	redrawOn(
		view.scrollFrames,
		view.resolution,
		scrollY,
		tl,
		clipPaint,
		rowLayout,
		cutFrames,
		store.currentTime,
		cssW,
		cssH,
		totalFrames,
		fps,
		filmstripVersion,
		store.waveform,
	);
});

const playheadLabel = $derived(
	`Playhead at ${store.currentTime.toFixed(2)} of ${outputDurationSec.toFixed(2)} seconds`,
);
</script>

<div
	bind:this={containerEl}
	class="flex h-full min-h-0 w-full overflow-hidden rounded-lg border border-border/60 bg-background text-[12px]"
	id="timeline-container"
>
	<div
		class="relative shrink-0 overflow-hidden border-r border-border/60 bg-background"
		style="width:{TRACK_HEADER_W}px"
		id="timeline-header"
	>
		<div
			class="flex items-center justify-between gap-2 border-b border-border/60 pl-1.5 pr-2.5"
			style="height:{RULER_HEIGHT}px"
			id="timeline-controls"
		>
			<div class="flex shrink-0 items-center gap-0.5">
				<Button
					type="button"
					onclick={togglePlay}
					disabled={!videoEl}
					aria-label={store.isPlaying ? "Pause" : "Play"}
					class="tl-btn"
					size="icon-sm"
					variant="ghost"
				>
					{#if store.isPlaying}<Pause
							fill="currentColor"
						/>{:else}<Play fill="currentColor" />{/if}
				</Button>
				<Button
					type="button"
					onclick={splitAtPlayhead}
					disabled={totalFrames <= 0}
					aria-label="Split at playhead"
					title="Split at playhead (S)"
					class="tl-btn"
					size="icon-sm"
					variant="ghost"
				>
					<SquareSplitHorizontal />
				</Button>
				<DropdownMenu.Root>
					<DropdownMenu.Trigger>
						<Button
							type="button"
							aria-label="More timeline options"
							class="tl-btn"
							size="icon-sm"
							variant="ghost"
						>
							<Ellipsis />
						</Button>
					</DropdownMenu.Trigger>
					<DropdownMenu.Content
						size="sm"
						align="start"
						class="max-w-44 w-fit"
					>
						<DropdownMenu.Item
							onSelect={addZoomAtPlayhead}
							disabled={totalFrames <= 0}
							size="sm"
						>
							<ZoomIn />
							Add zoom (Z)
						</DropdownMenu.Item>
						<DropdownMenu.Item onSelect={zoomToFit} size="sm">
							<Maximize2 />
							Zoom to fit (F)
						</DropdownMenu.Item>
						<DropdownMenu.Separator />
						<DropdownMenu.Sub>
							<DropdownMenu.SubTrigger size="sm">
								<Scissors />
								Trim
							</DropdownMenu.SubTrigger>
							<DropdownMenu.SubContent>
								<DropdownMenu.Item onSelect={markIn} size="sm"
									>Mark in ([)</DropdownMenu.Item
								>
								<DropdownMenu.Item onSelect={markOut} size="sm"
									>Mark out (])</DropdownMenu.Item
								>
								<DropdownMenu.Item
									onSelect={resetTrim}
									size="sm">Use full clip</DropdownMenu.Item
								>
							</DropdownMenu.SubContent>
						</DropdownMenu.Sub>
						<DropdownMenu.Sub>
							<DropdownMenu.SubTrigger size="sm">
								<Clock />
								Time display
							</DropdownMenu.SubTrigger>
							<DropdownMenu.SubContent>
								<DropdownMenu.CheckboxItem
									checked={store.timeMode === "smpte"}
									onCheckedChange={() =>
										(store.timeMode = "smpte")}
									size="sm"
								>
									Timecode
								</DropdownMenu.CheckboxItem>
								<DropdownMenu.CheckboxItem
									checked={store.timeMode === "seconds"}
									onCheckedChange={() =>
										(store.timeMode = "seconds")}
									size="sm"
								>
									Seconds
								</DropdownMenu.CheckboxItem>
								<DropdownMenu.CheckboxItem
									checked={store.timeMode === "frames"}
									size="sm"
									onCheckedChange={() =>
										(store.timeMode = "frames")}
								>
									Frames
								</DropdownMenu.CheckboxItem>
							</DropdownMenu.SubContent>
						</DropdownMenu.Sub>
						<DropdownMenu.Sub>
							<DropdownMenu.SubTrigger size="sm">
								<Upload />
								Apply on export
							</DropdownMenu.SubTrigger>
							<DropdownMenu.SubContent>
								<DropdownMenu.CheckboxItem
									checked={store.cutsEnabled}
									onCheckedChange={(v) =>
										(store.cutsEnabled = v)}
									size="sm"
								>
									Silence cuts
								</DropdownMenu.CheckboxItem>
								<DropdownMenu.CheckboxItem
									checked={!store.annotationsGloballyHidden}
									size="sm"
									onCheckedChange={(v) =>
										(store.annotationsGloballyHidden = !v)}
								>
									Markup
								</DropdownMenu.CheckboxItem>
							</DropdownMenu.SubContent>
						</DropdownMenu.Sub>
						<DropdownMenu.Separator />
						<DropdownMenu.CheckboxItem
							checked={loopEnabled}
							onCheckedChange={(v) => (loopEnabled = v)}
							size="sm"
						>
							<Repeat />
							Loop playback
						</DropdownMenu.CheckboxItem>
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			</div>
			<span
				class="min-w-0 truncate font-mono text-xs font-thin tabular-nums text-foreground"
			>
				{timecode(playheadFrame)}
			</span>
		</div>
		<div
			class="absolute inset-x-0 overflow-hidden"
			style="top:{RULER_HEIGHT}px;bottom:0"
		>
			<div style={headerScrollStyle}>
				{#each rowLayout as laid (laid.row.id)}
					{@const row = laid.row}
					{@const Icon = LANE_ICONS[row.kind]}
					{@const spec = PROP[row.kind]}
					<div style="margin-bottom:{ROW_GAP}px">
						<div
							class="group/row flex items-center justify-between gap-1 pl-1 pr-1.5 text-muted-foreground transition-colors {rowSelected(
								row,
							)
								? 'bg-accent'
								: 'hover:bg-accent/70'}"
							style="height:{laid.height}px"
						>
							<div class="flex min-w-0 items-center gap-0.5">
								{#if laid.expandable}
									<button
										type="button"
										class="flex size-4 shrink-0 items-center justify-center rounded-sm opacity-0 transition-opacity group-hover/row:opacity-100 hover:text-foreground"
										aria-expanded={laid.expanded}
										aria-label={laid.expanded
											? "Collapse"
											: "Expand"}
										onclick={() => toggleExpand(row.id)}
									>
										{#if laid.expanded}<ChevronDown
												class="size-3.5"
											/>{:else}<ChevronRight
												class="size-3.5"
											/>{/if}
									</button>
								{:else}
									<span class="w-4 shrink-0"></span>
								{/if}
								<Icon class="size-4 shrink-0" />
								<span
									class="truncate px-0.5 text-xs text-foreground"
									>{row.label}</span
								>
							</div>
							{#if rowToggleKind(row)}
								{@const on = rowToggled(row)}
								{@const mute = rowToggleKind(row) === "mute"}
								<button
									type="button"
									class="flex size-6 shrink-0 items-center justify-center rounded-sm transition-opacity hover:text-foreground {on
										? 'opacity-100'
										: 'opacity-0 group-hover/row:opacity-100'}"
									aria-pressed={on}
									aria-label={mute
										? on
											? "Unmute lane"
											: "Mute lane"
										: on
											? "Show lane"
											: "Hide lane"}
									onclick={() => toggleRow(row)}
								>
									{#if mute}
										{#if on}<VolumeOff
												class="size-3.5"
											/>{:else}<Volume2
												class="size-3.5"
											/>{/if}
									{:else if on}<EyeOff
											class="size-3.5"
										/>{:else}<Eye class="size-3.5" />{/if}
								</button>
							{/if}
						</div>
						{#if laid.expanded && spec}
							{@const PropIcon = spec.icon}
							<div
								class="flex items-center pl-6 pr-1.5"
								style="height:{SUBROW_H}px"
							>
								<div
									class="flex h-8 min-w-0 flex-1 items-center gap-1.5 rounded-lg bg-muted/60 pl-2 pr-2.5 ring-1 ring-inset ring-border/40 transition-colors hover:bg-muted"
								>
									<PropIcon
										class="size-3.5 shrink-0 text-muted-foreground"
									/>
									<input
										type="range"
										class="tl-range"
										min={spec.min}
										max={spec.max}
										step={spec.step}
										value={propValue(row)}
										oninput={(e) =>
											setProp(
												row,
												e.currentTarget.valueAsNumber,
											)}
										aria-label={`${row.label} ${spec.label}`}
									/>
									<span
										class="shrink-0 font-mono text-[11px] tabular-nums text-foreground/85"
										style="min-width:2.5rem;text-align:right"
									>
										{spec.format(propValue(row))}
									</span>
								</div>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		</div>
	</div>

	<!-- The canvas: ruler, rows, clips, playhead. -->
	<ContextMenu.Root>
		<ContextMenu.Trigger>
			{#snippet child({ props })}
				<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
				<div
					{...props}
					class="relative min-w-0 flex-1 outline-none focus-visible:ring-1 focus-visible:ring-ring/50"
					role="application"
					aria-label="Timeline. Home and End jump to the start and end; drag the ruler to scrub, drag a clip to move or trim it."
					tabindex="0"
					onkeydown={onKeyDown}
					oncontextmenucapture={onContextMenu}
				>
					<canvas
						bind:this={canvasEl}
						class="block size-full touch-none select-none"
						onpointerdown={onPointerDown}
						onpointermove={onPointerMove}
						onpointerup={onPointerUp}
						onpointercancel={onPointerUp}
						onpointerleave={onPointerLeave}
						onwheel={onWheel}
					></canvas>
					<span class="sr-only" aria-live="polite"
						>{playheadLabel}</span
					>
				</div>
			{/snippet}
		</ContextMenu.Trigger>
		<ContextMenu.Content size="sm" class="w-44">
			{#if menuClip}
				<ContextMenu.Label>{menuClip.label}</ContextMenu.Label>
				{#if menuCanEdit}
					<ContextMenu.Separator />
					<ContextMenu.Item onSelect={menuDuplicate}>
						<Copy />
						Duplicate
					</ContextMenu.Item>
					<ContextMenu.Item onSelect={menuToggleHidden}>
						{#if menuClip.hidden}<Eye />Show{:else}<EyeOff
							/>Hide{/if}
					</ContextMenu.Item>
				{/if}
				{#if menuClip.kind === "markup"}
					<ContextMenu.Item onSelect={menuToggleLock}>
						{#if menuClip.locked}<Unlock />Unlock{:else}<Lock
							/>Lock{/if}
					</ContextMenu.Item>
				{/if}
				{#if menuClip.kind === "video"}
					<ContextMenu.Separator />
					<ContextMenu.Item onSelect={splitAtPlayhead}>
						<SquareSplitHorizontal />
						Split at playhead
					</ContextMenu.Item>
					<ContextMenu.Label
						>Speed · {menuSpeed.toFixed(2)}×</ContextMenu.Label
					>
					{#each SPEED_PRESETS as sp (sp)}
						<ContextMenu.Item onSelect={() => menuSetSpeed(sp)}>
							<Gauge />
							{sp}×
						</ContextMenu.Item>
					{/each}
				{/if}
				{#if menuCanDelete}
					<ContextMenu.Separator />
					<ContextMenu.Item
						variant="destructive"
						onSelect={menuDelete}
					>
						<Trash2 />
						{menuClip.kind === "video"
							? "Delete (ripple)"
							: "Delete"}
					</ContextMenu.Item>
				{/if}
			{:else}
				<ContextMenu.Item disabled size="sm" class="text-muted-foreground font-light"
					>Right-click a clip to edit it</ContextMenu.Item
				>
			{/if}
		</ContextMenu.Content>
	</ContextMenu.Root>
</div>

<style>
	/* Transport icon button, matching the panels' ghost icon-button. */
	.tl-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.75rem;
		height: 1.75rem;
		border-radius: calc(var(--radius) - 4px);
		color: var(--muted-foreground);
		transition:
			background-color 0.15s,
			color 0.15s;
	}
	.tl-btn:hover:not(:disabled) {
		color: var(--foreground);
		background: color-mix(in oklab, var(--muted) 50%, transparent);
	}
	.tl-btn:disabled {
		opacity: 0.4;
	}
	/* Dense slider on the shared field surface: --primary pill on a muted track. */
	.tl-range {
		flex: 1;
		min-width: 0;
		height: 4px;
		appearance: none;
		border-radius: 999px;
		background: color-mix(in oklab, var(--foreground) 10%, transparent);
		cursor: ew-resize;
	}
	.tl-range::-webkit-slider-thumb {
		appearance: none;
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: var(--primary);
		box-shadow: 0 0 0 1px
			color-mix(in oklab, var(--background) 60%, transparent);
	}
	.tl-range::-moz-range-thumb {
		width: 12px;
		height: 12px;
		border: 0;
		border-radius: 50%;
		background: var(--primary);
	}
</style>
