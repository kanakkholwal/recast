// Throwaway adapter: turns today's heterogeneous model (segments,
// zoom, annotations, captions, audio) into the ONE row tree the canvas timeline
// renders — one row per clip, grouped by type, empty groups omitted, voice and
// music folded into a single "Audio" type. Pure and snapshot-driven so it
// unit-tests without a store. Deleted when the Rust node tree feeds rows itself.

import { originalToOutput, type TimeMap } from "./time-map";

export type ClipKind = "video" | "audio" | "zoom" | "markup" | "caption";

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
}

/** One header row and the clip(s) drawn on its canvas track. Most rows hold a
 *  single clip; the captions row holds every caption segment. */
export interface TimelineRow {
	id: string;
	kind: ClipKind;
	label: string;
	clips: TimelineClip[];
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

/** Item already timed on the OUTPUT axis (captions, audio clips). */
interface OutputItem {
	id: string;
	start: number;
	end: number;
	label: string;
	selected?: boolean;
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
	};
}

/** Build the row tree: ONE row per type (all its clips laid side by side), in
 *  order Video, Zoom, Markup, Captions, Voice, Music. A type with no clips
 *  contributes no row, so the column only ever shows what the edit contains. */
export function buildTimelineRows(input: TimelineViewModelInput): TimelineRow[] {
	const rows: TimelineRow[] = [];

	if (input.segments.length > 0) {
		rows.push({
			id: "video",
			kind: "video",
			label: input.videoName || "Video",
			clips: input.segments.map((s) => originalClip("video", s, input)),
		});
	}
	if (input.zoomRegions.length > 0) {
		rows.push({
			id: "zoom",
			kind: "zoom",
			label: "Zoom",
			clips: input.zoomRegions.map((z) => originalClip("zoom", z, input)),
		});
	}
	if (input.annotations.length > 0) {
		rows.push({
			id: "markup",
			kind: "markup",
			label: "Markup",
			clips: input.annotations.map((a) => originalClip("markup", a, input)),
		});
	}
	if (input.captions.length > 0) {
		rows.push({
			id: "caption",
			kind: "caption",
			label: "Captions",
			clips: input.captions.map((c) => outputClip("caption", c, input.fps)),
		});
	}
	if (input.voiceClips.length > 0) {
		rows.push({
			id: "voice",
			kind: "audio",
			label: "Voice",
			clips: input.voiceClips.map((c) => outputClip("audio", c, input.fps)),
		});
	}
	if (input.musicClips.length > 0) {
		rows.push({
			id: "music",
			kind: "audio",
			label: "Music",
			clips: input.musicClips.map((c) => outputClip("audio", c, input.fps)),
		});
	}

	return rows;
}
