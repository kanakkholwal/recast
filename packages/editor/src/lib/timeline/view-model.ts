// Throwaway adapter: today's model (segments/zoom/annotations/captions/audio) into the one canvas row tree (grouped by type, voice+music folded to Audio); pure/snapshot-driven, retired when the Rust node tree feeds rows.

import { originalToOutput, type TimeMap } from "./time-map";

export type ClipKind = "video" | "audio" | "zoom" | "markup" | "caption" | "camera";

export interface TimelineClip {
	id: string;
	kind: ClipKind;
	/** OUTPUT frames. */
	start: number;
	/** OUTPUT frames. */
	duration: number;
	label: string;
	selected: boolean;
	hidden: boolean;
	locked: boolean;
	/** Stack level within the row: overlapping clips get distinct lanes. */
	lane: number;
}

/** A single keyframe on a property track, positioned in OUTPUT frames. */
export interface TimelineKeyframe {
	/** OUTPUT frames. */
	frame: number;
	selected: boolean;
}

/** A property animated over time, drawn as diamonds rather than clip bars.
 *  `source` names the model/property so the host maps it back to store edits,
 *  keeping the timeline generic across whatever gains keyframes next. */
export interface TimelineTrack {
	source: string;
	keyframes: TimelineKeyframe[];
}

/** One header row and the clip(s) drawn on its canvas track. Most rows hold a
 *  single clip; the captions row holds every caption segment; a track row holds
 *  keyframes instead of clips. */
export interface TimelineRow {
	id: string;
	kind: ClipKind;
	label: string;
	clips: TimelineClip[];
	/** How many stack levels the clips occupy (1 when none overlap). */
	laneCount: number;
	/** Present when the row is a keyframe track (mutually exclusive with clips). */
	track?: TimelineTrack;
}

/** First-fit interval packing: each clip drops to the lowest lane whose previous
 *  clip has already ended, so time-overlapping clips never share a lane. Mutates
 *  `lane` in place and returns the lane count. */
export function assignLanes(clips: TimelineClip[]): number {
	const laneEnds: number[] = [];
	for (const clip of [...clips].sort((a, b) => a.start - b.start)) {
		let lane = laneEnds.findIndex((end) => clip.start >= end);
		if (lane === -1) {
			lane = laneEnds.length;
			laneEnds.push(0);
		}
		clip.lane = lane;
		laneEnds[lane] = clip.start + clip.duration;
	}
	return Math.max(1, laneEnds.length);
}

/** Item timed on the ORIGINAL recording axis (segments, zoom, annotations). */
interface OriginalItem {
	id: string;
	start: number;
	end: number;
	label: string;
	selected?: boolean;
	hidden?: boolean;
	locked?: boolean;
}

/** Item already timed on the OUTPUT axis (voice and music clips). */
interface OutputItem {
	id: string;
	start: number;
	end: number;
	label: string;
	selected?: boolean;
}

/** A keyframe track fed in by the host: keyframe times on the ORIGINAL axis,
 *  plus which one (if any) is selected, so the row can highlight it. */
export interface TrackItem {
	id: string;
	source: string;
	label: string;
	kind: ClipKind;
	/** ORIGINAL-axis seconds. */
	times: number[];
	selectedTime?: number | null;
}

export interface TimelineViewModelInput {
	fps: number;
	/** Maps original-axis seconds to output-axis seconds. */
	map: TimeMap;
	/** Name shown on every video row (the recording file). */
	videoName: string;
	segments: OriginalItem[];
	zoomRegions: OriginalItem[];
	annotations: OriginalItem[];
	captions: OutputItem[];
	voiceClips: OutputItem[];
	musicClips: OutputItem[];
	/** Keyframe tracks (camera today; generic for future animated models). */
	tracks?: TrackItem[];
}

function originalClip(
	kind: ClipKind,
	it: OriginalItem,
	input: TimelineViewModelInput,
): TimelineClip {
	const startSec = originalToOutput(input.map, it.start);
	const endSec = originalToOutput(input.map, it.end);
	return {
		id: it.id,
		kind,
		start: startSec * input.fps,
		duration: Math.max(0, (endSec - startSec) * input.fps),
		label: it.label,
		selected: it.selected ?? false,
		hidden: it.hidden ?? false,
		locked: it.locked ?? false,
		lane: 0,
	};
}

function outputClip(kind: ClipKind, it: OutputItem, fps: number): TimelineClip {
	return {
		id: it.id,
		kind,
		start: it.start * fps,
		duration: Math.max(0, (it.end - it.start) * fps),
		label: it.label,
		selected: it.selected ?? false,
		hidden: false,
		locked: false,
		lane: 0,
	};
}

function makeRow(id: string, kind: ClipKind, label: string, clips: TimelineClip[]): TimelineRow {
	return { id, kind, label, clips, laneCount: assignLanes(clips) };
}

/** Build the row tree: ONE row per type (all its clips laid side by side), in
 *  order Video, Zoom, Markup, Captions, Voice, Music. A type with no clips
 *  contributes no row, so the column only ever shows what the edit contains. */
export function buildTimelineRows(input: TimelineViewModelInput): TimelineRow[] {
	const rows: TimelineRow[] = [];

	if (input.segments.length > 0) {
		const clips = input.segments.map((s) => originalClip("video", s, input));
		rows.push(makeRow("video", "video", input.videoName || "Video", clips));
	}
	if (input.zoomRegions.length > 0) {
		const clips = input.zoomRegions.map((z) => originalClip("zoom", z, input));
		rows.push(makeRow("zoom", "zoom", "Zoom", clips));
	}
	if (input.annotations.length > 0) {
		const clips = input.annotations.map((a) => originalClip("markup", a, input));
		rows.push(makeRow("markup", "markup", "Markup", clips));
	}
	if (input.captions.length > 0) {
		// ORIGINAL axis: `captionTranscript` is timed against the recording, which is what CaptionOverlay resolves it on. Voice and music clips really are output-timed, so they keep `outputClip`.
		const clips = input.captions.map((c) => originalClip("caption", c, input));
		rows.push(makeRow("caption", "caption", "Captions", clips));
	}
	if (input.voiceClips.length > 0) {
		const clips = input.voiceClips.map((c) => outputClip("audio", c, input.fps));
		rows.push(makeRow("voice", "audio", "Voice", clips));
	}
	if (input.musicClips.length > 0) {
		const clips = input.musicClips.map((c) => outputClip("audio", c, input.fps));
		rows.push(makeRow("music", "audio", "Music", clips));
	}
	for (const t of input.tracks ?? []) {
		// Keep a track with no keyframes: the caller only emits ones that should show (an enabled camera renders a bare baseline).
		const keyframes: TimelineKeyframe[] = t.times.map((sec) => ({
			frame: originalToOutput(input.map, sec) * input.fps,
			selected: t.selectedTime != null && Math.abs(sec - t.selectedTime) < 1e-3,
		}));
		rows.push({
			id: t.id,
			kind: t.kind,
			label: t.label,
			clips: [],
			laneCount: 1,
			track: { source: t.source, keyframes },
		});
	}

	return rows;
}
