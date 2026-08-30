<script lang="ts">
import {
	Captions,
	ChevronDown,
	ChevronRight,
	Copy,
	Ellipsis,
	Eye,
	EyeOff,
	Film,
	Gauge,
	Highlighter,
	Lock,
	Mic,
	Pause,
	Play,
	Repeat,
	Scissors,
	SquareSplitHorizontal,
	Sun,
	Trash2,
	Unlock,
	Volume2,
	ZoomIn,
} from "@recast/icons";
import * as ContextMenu from "@recast/ui/context-menu";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { onMount } from "svelte";
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
import type { Storyboard, TileProvider } from "../../../lib/timeline/filmstrip-source";
import { originalToOutput, outputToOriginal } from "../../../lib/timeline/time-map";
import {
	buildTimelineRows,
	type ClipKind,
	type TimelineClip,
	type TimelineRow,
} from "../../../lib/timeline/view-model";
import { clipEndSec } from "../../../lib/audio/music";
import { kindLabel } from "../../../lib/annotations/kind-label";
import type { EditorStore } from "../../../stores/editor-store.svelte";
import { effectiveFps } from "./timeline-helpers";

// Canvas timeline, ported from Diffusion Studio to our object model: a dark NLE
// surface, one ROW PER CLIP grouped by type (empty types omitted, voice+music
// folded into Audio), a header column that lists each row and expands to its
// property controls, frame-snapped move/trim, cut seams, captions and a shadcn
// context menu. View state (scroll/zoom/expansion) is local — never the file.

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
const ROW_H = 44;
const SUBROW_H = 30;
const ROW_GAP = 3;
const CLIP_RADIUS = 4;
const CLIP_LABEL_X = 6;
const CLIP_LABEL_Y = 13;
const CLIP_LABEL_HEIGHT = 20;
const CLIP_SM = 40;
const TRIM_HANDLE_PX = 10;
const SNAP_PX = 10;
const MIN_CLIP_FRAMES = 2;
const TRACK_HEADER_W = 184;
const SAMPLE_WIDTH = 2;
// The header floats over the canvas's left edge, so the canvas element stays
// full width (responsive) and its time content begins after this gutter.
const GUTTER = TRACK_HEADER_W;

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
		icon: Gauge,
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
};

let containerEl: HTMLDivElement | undefined = $state();
let canvasEl: HTMLCanvasElement | undefined = $state();
let ctx: CanvasRenderingContext2D | null = null;

let cssW = $state(900);
let cssH = $state(200);
let dpr = 1;
let didFit = false;

// Width available for the time content, i.e. everything right of the header gutter.
const contentW = $derived(Math.max(1, cssW - GUTTER));

let view = $state<TimelineView>({ scrollFrames: 0, resolution: DEFAULT_RESOLUTION });
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
	expandable: boolean;
	expanded: boolean;
}
const rowLayout = $derived.by<LaidRow[]>(() => {
	let top = ROW_GAP;
	const out: LaidRow[] = [];
	for (const row of rows) {
		const expandable = PROP[row.kind] !== undefined && row.clips.length >= 1;
		const expanded = expandable && expandedRows.has(row.id);
		out.push({ row, top, expandable, expanded });
		top += ROW_H + (expanded ? SUBROW_H : 0) + ROW_GAP;
	}
	return out;
});
const contentHeight = $derived(
	rowLayout.reduce((h, l) => h + ROW_H + (l.expanded ? SUBROW_H : 0) + ROW_GAP, ROW_GAP),
);
const allClips = $derived(rows.flatMap((r) => r.clips));

// Cut seams (silence removed): each collapses to one OUTPUT frame where its
// original range folds shut. Drawn as a full-height notch, not a clip.
const cutFrames = $derived.by<number[]>(() => {
	if (!store.cutsEnabled) return [];
	return store.cuts.map((c) => originalToOutput(store.renderMap, c.start) * fps);
});

function toggleExpand(id: string) {
	const next = new Set(expandedRows);
	if (next.has(id)) next.delete(id);
	else next.add(id);
	expandedRows = next;
	scrollY = clampScrollY(scrollY);
}

// The property edits the SELECTED clip of a multi-clip row, or the first one.
function propClipId(row: TimelineRow): string | undefined {
	return (row.clips.find((c) => c.selected) ?? row.clips[0])?.id;
}
function propValue(row: TimelineRow): number {
	const id = propClipId(row);
	if (row.kind === "audio") return store.musicClips.find((c) => c.id === id)?.gain ?? 100;
	if (row.kind === "zoom") return store.zoomRegions.find((z) => z.id === id)?.scale ?? 1;
	if (row.kind === "markup") return store.annotations.find((a) => a.id === id)?.opacity ?? 1;
	return 0;
}
function setProp(row: TimelineRow, v: number) {
	const id = propClipId(row);
	if (!id) return;
	if (row.kind === "audio") store.updateMusicClip(id, { gain: v });
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
	if (!didFit && totalFrames > 0) {
		view = clampScroll(
			zoomAt({ scrollFrames: 0, resolution: (contentW * 0.98) / totalFrames }, 0, 1),
			totalFrames,
			contentW,
		);
		didFit = true;
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
		const groupH = ROW_H + (laid.expanded ? SUBROW_H : 0);
		if (top + groupH < RULER_HEIGHT || top > cssH) continue;
		// Row band.
		ctx.fillStyle = tl.surfaceMuted;
		ctx.globalAlpha = i % 2 === 0 ? 0.5 : 0.32;
		roundRectPath(0, top, contentW, ROW_H, CLIP_RADIUS);
		ctx.fill();
		if (laid.expanded) {
			// Property track band, a touch dimmer than the clip band.
			ctx.globalAlpha = 0.22;
			roundRectPath(0, top + ROW_H, contentW, SUBROW_H, CLIP_RADIUS);
			ctx.fill();
		}
		ctx.globalAlpha = 1;
		for (const clip of laid.row.clips) drawClip(clip, top);
	}
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

function drawClip(clip: TimelineClip, top: number) {
	if (!ctx) return;
	const x = frameToX(clip.start, view);
	const w = Math.max(2, clip.duration * view.resolution);
	if (x + w < 0 || x > contentW) return;
	const paint = clipPaint[clip.kind];
	const h = ROW_H;

	ctx.globalAlpha = clip.hidden ? 0.5 : 1;

	ctx.fillStyle = paint.bg;
	roundRectPath(x, top, w, h, CLIP_RADIUS);
	ctx.fill();

	if (clip.kind === "video") {
		drawFilmstrip(x, w, top, h);
		drawWaveformBars({ x, w, y: top, h }, paint.primary, true);
	} else if (clip.kind === "audio") {
		drawWaveformBars({ x, w, y: top, h }, paint.primary, false);
	}

	if (w >= 24) {
		ctx.save();
		roundRectPath(x, top, w, h, CLIP_RADIUS);
		ctx.clip();
		ctx.fillStyle = paint.on;
		ctx.font = "11px Inter, system-ui, sans-serif";
		ctx.textBaseline = "middle";
		ctx.textAlign = "left";
		ctx.fillText(
			truncate(clip.label, w - 2 * CLIP_LABEL_X),
			x + CLIP_LABEL_X,
			top + (h >= 32 ? CLIP_LABEL_Y : h / 2),
		);
		ctx.restore();
	}

	if (clip.selected && canTrim(clip) && w > TRIM_HANDLE_PX * 2) {
		ctx.fillStyle = paint.on;
		ctx.globalAlpha = 0.55;
		ctx.fillRect(x + 2, top + h / 2 - 6, 1.5, 12);
		ctx.fillRect(x + w - 3.5, top + h / 2 - 6, 1.5, 12);
		ctx.globalAlpha = clip.hidden ? 0.5 : 1;
	}

	ctx.save();
	roundRectPath(x, top, w, h, CLIP_RADIUS);
	ctx.clip();
	ctx.strokeStyle = clip.selected ? tl.ring : tl.border;
	ctx.lineWidth = clip.selected ? 2 : 1;
	roundRectPath(x, top, w, h, CLIP_RADIUS);
	ctx.stroke();
	ctx.restore();

	ctx.globalAlpha = 1;
}

function drawFilmstrip(x0: number, cw: number, y: number, h: number): void {
	if (!ctx || !ensureStoryboard() || !sbImg || !sbMeta) return;
	const meta = sbMeta;
	const img = sbImg;
	const tileW = Math.max(8, h * (meta.cellW / meta.cellH));
	ctx.save();
	roundRectPath(x0, y, cw, h, CLIP_RADIUS);
	ctx.clip();
	ctx.globalAlpha = 0.92;
	for (let tx = 0; tx < cw; tx += tileW) {
		const outSec = xToFrame(x0 + tx + tileW / 2, view) / fps;
		const origSec = outputToOriginal(store.renderMap, outSec);
		const i = Math.max(
			0,
			Math.min(
				meta.count - 1,
				Math.floor((origSec / Math.max(meta.durationSec, 1e-6)) * meta.count),
			),
		);
		const sx = (i % meta.cols) * meta.cellW;
		const sy = Math.floor(i / meta.cols) * meta.cellH;
		ctx.drawImage(img, sx, sy, meta.cellW, meta.cellH, x0 + tx, y, tileW, h);
	}
	ctx.globalAlpha = 1;
	ctx.restore();
}

function drawWaveformBars(r: ClipRect, color: string, anchorBottom: boolean): void {
	if (!ctx) return;
	const wf = store.waveform;
	const dur = store.metadata?.duration ?? 0;
	if (!wf || wf.length < 2 || dur <= 0) return;

	ctx.save();
	roundRectPath(r.x, r.y, r.w, r.h, CLIP_RADIUS);
	ctx.clip();
	ctx.fillStyle = color;
	ctx.globalAlpha = anchorBottom ? 0.7 : 1;

	const offsetY = r.h > CLIP_SM ? CLIP_LABEL_HEIGHT : 2;
	const bandH = Math.max(2, r.h - offsetY - 3);
	const inSec = store.inPoint;
	const outSec = store.outPoint;

	for (let sx = 0; sx < r.w; sx += SAMPLE_WIDTH) {
		const oSec = xToFrame(r.x + sx, view) / fps;
		const origSec = outputToOriginal(store.renderMap, oSec);
		if (origSec < inSec - 0.01 || origSec > outSec + 0.01) continue;
		const i = Math.max(0, Math.min(wf.length - 1, Math.floor((origSec / dur) * wf.length)));
		const bh = Math.max((wf[i] ?? 0) * bandH, 1);
		const top = anchorBottom ? r.y + r.h - 3 - bh : r.y + offsetY + (bandH - bh) / 2;
		ctx.fillRect(r.x + sx, top, SAMPLE_WIDTH - 0.5, bh);
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
	const x = frameToX(playheadFrame, view);
	if (x < -2 || x > contentW + 2) return;

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

	// The line below the ruler: a dark halo (3px) with the blue scrubber over it.
	ctx.beginPath();
	ctx.moveTo(0.5, RULER_HEIGHT);
	ctx.lineTo(0.5, cssH);
	ctx.strokeStyle = tl.border;
	ctx.lineWidth = 3;
	ctx.stroke();
	ctx.strokeStyle = tl.ring;
	ctx.lineWidth = 1;
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
	grad.addColorStop(0, `${tl.ring}00`);
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
	| { kind: "move"; clip: TimelineClip; grabFrame: number; startFrame: number; durFrames: number }
	| { kind: "trim"; edge: "l" | "r"; clip: TimelineClip; startFrame: number; endFrame: number };
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
		if (y < top || y >= top + ROW_H) continue;
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
	if (y < RULER_HEIGHT) return "ew-resize";
	const clip = clipAt(x, y);
	if (!clip) return "default";
	if (clip.kind === "video" && !videoTrimmable) return "pointer";
	if (!canMove(clip) && !canTrim(clip)) return "pointer";
	const zone = edgeZone(clip, x);
	if (zone !== "body") return "ew-resize";
	return canMove(clip) ? "grab" : "pointer";
}

function onPointerDown(e: PointerEvent) {
	if (e.button !== 0) return;
	const x = localX(e);
	const y = localY(e);
	if (y < RULER_HEIGHT) {
		if (store.isPlaying) {
			videoEl?.pause();
			store.isPlaying = false;
		}
		gesture = { kind: "scrub" };
		canvasEl?.setPointerCapture(e.pointerId);
		seekToX(x);
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
const menuCanEdit = $derived(menuClip?.kind === "zoom" || menuClip?.kind === "markup");
const menuCanDelete = $derived(
	menuClip != null &&
		(menuClip.kind === "zoom" || menuClip.kind === "markup" || menuClip.kind === "audio"),
);

function onWheel(e: WheelEvent) {
	e.preventDefault();
	if (e.ctrlKey || e.metaKey) {
		const clamped = Math.max(-ZOOM_DELTA_CLAMP, Math.min(ZOOM_DELTA_CLAMP, e.deltaY));
		view = clampScroll(
			zoomAt(view, localX(e), Math.exp(-clamped * ZOOM_SENSITIVITY)),
			totalFrames,
			contentW,
		);
		return;
	}
	const dxDominant = Math.abs(e.deltaX) > Math.abs(e.deltaY);
	if (!e.shiftKey && !dxDominant && maxScrollY() > 0) {
		scrollY = clampScrollY(scrollY + e.deltaY);
		return;
	}
	const dx = (dxDominant ? e.deltaX : e.deltaY) * SCROLL_X_SENSITIVITY;
	view = clampScroll(scrollByPixels(view, dx), totalFrames, contentW);
}

function onKeyDown(e: KeyboardEvent) {
	if (e.key === "Home") {
		e.preventDefault();
		store.seek(0);
	} else if (e.key === "End") {
		e.preventDefault();
		store.seek(outputToOriginal(store.renderMap, outputDurationSec));
	}
}

function timecode(frame: number): string {
	const f = Math.max(0, Math.round(frame));
	const perSec = fps > 0 ? fps : 60;
	const ff = (f % perSec).toString().padStart(2, "0");
	const total = Math.floor(f / perSec);
	const mm = Math.floor(total / 60)
		.toString()
		.padStart(2, "0");
	const ss = (total % 60).toString().padStart(2, "0");
	return `${mm}:${ss}:${ff}`;
}

// The header column scrolls its rows with the canvas.
const headerScrollStyle = $derived(`transform:translateY(${-scrollY}px);padding-top:${ROW_GAP}px`);

onMount(() => {
	ctx = canvasEl?.getContext("2d", { alpha: false }) ?? null;
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
	};
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
	class="relative h-full min-h-0 w-full overflow-hidden rounded-lg border border-border/60 bg-background text-[12px]"
>
	<!-- Track/properties panel: floats over the canvas's left gutter so the canvas
	     itself stays full width and responsive. Scrolls its rows with the canvas. -->
	<div
		class="absolute inset-y-0 left-0 z-10 overflow-hidden border-r border-border/60 bg-background"
		style="width:{TRACK_HEADER_W}px"
	>
		<div
			class="flex items-center justify-between gap-2 border-b border-border/60 pl-1.5 pr-2.5"
			style="height:{RULER_HEIGHT}px"
		>
			<div class="flex items-center gap-0.5">
				<button
					type="button"
					onclick={togglePlay}
					disabled={!videoEl}
					aria-label={store.isPlaying ? "Pause" : "Play"}
					class="tl-btn"
				>
					{#if store.isPlaying}<Pause class="size-4" fill="currentColor" />{:else}<Play
							class="size-4"
							fill="currentColor"
						/>{/if}
				</button>
				<button
					type="button"
					onclick={splitAtPlayhead}
					disabled={totalFrames <= 0}
					aria-label="Split at playhead"
					title="Split at playhead"
					class="tl-btn"
				>
					<SquareSplitHorizontal class="size-4" />
				</button>
				<DropdownMenu.Root>
					<DropdownMenu.Trigger>
						<button type="button" aria-label="More timeline options" class="tl-btn">
							<Ellipsis class="size-4" />
						</button>
					</DropdownMenu.Trigger>
					<DropdownMenu.Content size="sm" align="start" class="w-44">
						<DropdownMenu.CheckboxItem checked={loopEnabled} onCheckedChange={(v) => (loopEnabled = v)}>
							<Repeat class="size-3.5" />
							Loop playback
						</DropdownMenu.CheckboxItem>
						<DropdownMenu.CheckboxItem
							checked={store.cutsEnabled}
							onCheckedChange={(v) => (store.cutsEnabled = v)}
						>
							<Scissors class="size-3.5" />
							Apply silence cuts
						</DropdownMenu.CheckboxItem>
						<DropdownMenu.Separator />
						<DropdownMenu.Item onSelect={() => store.seek(0)}>Jump to start</DropdownMenu.Item>
						<DropdownMenu.Item
							onSelect={() => store.seek(outputToOriginal(store.renderMap, outputDurationSec))}
						>
							Jump to end
						</DropdownMenu.Item>
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			</div>
			<span class="font-mono text-[11px] tabular-nums text-foreground">
				{timecode(playheadFrame)}
			</span>
		</div>
		<div class="absolute inset-x-0 overflow-hidden" style="top:{RULER_HEIGHT}px;bottom:0">
			<div style={headerScrollStyle}>
				{#each rowLayout as laid (laid.row.id)}
					{@const row = laid.row}
					{@const Icon = LANE_ICONS[row.kind]}
					{@const spec = PROP[row.kind]}
					<div style="margin-bottom:{ROW_GAP}px">
						<div
							class="flex items-center gap-1.5 rounded-md px-1.5 transition-colors hover:bg-muted/40"
							style="height:{ROW_H}px"
						>
							{#if laid.expandable}
								<button
									type="button"
									class="flex size-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground/60 transition-colors hover:text-foreground"
									aria-expanded={laid.expanded}
									aria-label={laid.expanded ? "Collapse" : "Expand"}
									onclick={() => toggleExpand(row.id)}
								>
									{#if laid.expanded}<ChevronDown class="size-3" />{:else}<ChevronRight
											class="size-3"
										/>{/if}
								</button>
							{:else}
								<span class="w-5 shrink-0"></span>
							{/if}
							<Icon class="size-4 shrink-0" style="color:var(--tl-{row.kind}-primary)" />
							<span class="truncate text-[12px] font-medium text-foreground">{row.label}</span>
						</div>
						{#if laid.expanded && spec}
							{@const PropIcon = spec.icon}
							<div class="flex items-center pl-6 pr-1.5" style="height:{SUBROW_H}px">
								<div
									class="flex h-8 min-w-0 flex-1 items-center gap-1.5 rounded-lg bg-muted/60 pl-2 pr-2.5 ring-1 ring-inset ring-border/40 transition-colors hover:bg-muted"
								>
									<PropIcon class="size-3.5 shrink-0 text-muted-foreground" />
									<input
										type="range"
										class="tl-range"
										min={spec.min}
										max={spec.max}
										step={spec.step}
										value={propValue(row)}
										oninput={(e) => setProp(row, e.currentTarget.valueAsNumber)}
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
					class="absolute inset-0 outline-none focus-visible:ring-1 focus-visible:ring-ring/50"
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
					<span class="sr-only" aria-live="polite">{playheadLabel}</span>
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
						{#if menuClip.hidden}<Eye />Show{:else}<EyeOff />Hide{/if}
					</ContextMenu.Item>
				{/if}
				{#if menuClip.kind === "markup"}
					<ContextMenu.Item onSelect={menuToggleLock}>
						{#if menuClip.locked}<Unlock />Unlock{:else}<Lock />Lock{/if}
					</ContextMenu.Item>
				{/if}
				{#if menuCanDelete}
					<ContextMenu.Separator />
					<ContextMenu.Item variant="destructive" onSelect={menuDelete}>
						<Trash2 />
						Delete
					</ContextMenu.Item>
				{/if}
			{:else}
				<ContextMenu.Item disabled>Right-click a clip to edit it</ContextMenu.Item>
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
		box-shadow: 0 0 0 1px color-mix(in oklab, var(--background) 60%, transparent);
	}
	.tl-range::-moz-range-thumb {
		width: 12px;
		height: 12px;
		border: 0;
		border-radius: 50%;
		background: var(--primary);
	}
</style>
