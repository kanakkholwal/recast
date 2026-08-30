/**
 * Music / extra-audio clips placed on the OUTPUT timeline (over the finished
 * edit), distinct from the recording's own system/mic tracks. v1 imports local
 * files; the `MusicProvider` interface is the seam a future licensed-music API
 * (Uppbeat / Pixabay / Epidemic) plugs into — a provider result is cached into
 * the `.recast` bundle and stored as a `provider` source, so playback stays
 * offline-first (never a hotlink URL).
 */

import { tryGetEditorServices } from "../editor/services";

export interface AudioClipSourceLocal {
	kind: "local";
	/** Absolute path to an imported audio file. */
	path: string;
}

export interface AudioClipSourceProvider {
	kind: "provider";
	providerId: string;
	trackId: string;
	/** Local path of the downloaded asset (cached in the project bundle). */
	assetPath: string;
	attribution?: string;
	license?: string;
}

export type AudioClipSource = AudioClipSourceLocal | AudioClipSourceProvider;

/** `voice` clips are the recording's own audio, detached for independent editing;
 *  `music` is everything added on top. Both share this model and every render
 *  path — only the lane they live on and source-audio suppression differ. */
export type AudioClipRole = "music" | "voice";

/** A piece of audio laid on the output timeline (background music, voiceover…). */
export interface AudioClip {
	id: string;
	source: AudioClipSource;
	/** Omitted on legacy clips; treat as `music` (see {@link isVoiceClip}). */
	role?: AudioClipRole;
	/** Output-timeline start (seconds). Background music usually starts at 0. */
	startOutputSec: number;
	/** Trim into the source before playing (seconds). */
	offsetSec: number;
	/** Play length (seconds); `0` = to the end of the source (or, when looping,
	 *  fill the remaining output). */
	durationSec: number;
	/** Gain 0–200 (%). Music defaults below unity so it sits under the voice. */
	gain: number;
	muted: boolean;
	fadeIn: number; // seconds
	fadeOut: number; // seconds
	/** Repeat the source to fill `durationSec` (or the whole output). */
	loop: boolean;
	/** Auto-lower under the microphone (Phase 2B). */
	ducking: boolean;
}

export const DEFAULT_MUSIC_GAIN = 45;

/** Local path to fetch/decode/encode, whichever source kind it is. */
export function clipAssetPath(source: AudioClipSource): string {
	return source.kind === "local" ? source.path : source.assetPath;
}

/** Effective linear gain (0–1) with mute applied. */
export function clipGain(clip: AudioClip): number {
	if (clip.muted) return 0;
	return Math.max(0, Math.min(2, clip.gain / 100));
}

/** A ready-to-use clip for a freshly imported/added source: background music
 *  defaults — starts at 0, loops to fill, sits under the voice, short fades. */
export function defaultAudioClip(id: string, source: AudioClipSource): AudioClip {
	return {
		id,
		source,
		startOutputSec: 0,
		offsetSec: 0,
		durationSec: 0,
		gain: DEFAULT_MUSIC_GAIN,
		muted: false,
		fadeIn: 0.5,
		fadeOut: 1,
		loop: true,
		ducking: false,
	};
}

/** True for the recording's own detached audio (legacy clips default to music). */
export function isVoiceClip(clip: AudioClip): boolean {
	return clip.role === "voice";
}

/** A detached-recording-audio clip: the source's own file, played linearly from
 *  `offsetSec` at output 0, gain/mute carried from its per-source setting. */
export function voiceClip(id: string, path: string, over: Partial<AudioClip> = {}): AudioClip {
	return {
		...defaultAudioClip(id, { kind: "local", path }),
		role: "voice",
		gain: 100,
		loop: false,
		fadeIn: 0,
		fadeOut: 0,
		...over,
	};
}

/** Open an audio file picker. Returns the absolute path, or null if cancelled. */
export async function pickAudioFile(): Promise<string | null> {
	const pick = tryGetEditorServices()?.pickFile;
	if (!pick) return null;
	return await pick({
		accept: ["mp3", "wav", "m4a", "aac", "ogg", "flac"],
		title: "Add music or audio",
	});
}

/** Display name for a clip (the source file's basename). */
export function clipDisplayName(clip: AudioClip): string {
	const p = clipAssetPath(clip.source);
	const base = p.split(/[\\/]/).pop() ?? p;
	return base || "Audio";
}

// --- Timeline editing: output-axis and pure, shared with the interactive lane so preview and export follow for free.

/** Shortest a clip may be trimmed to (seconds). */
export const MIN_CLIP_DURATION = 0.1;

/** Played output length; `durationSec` 0 means "fill to the output end". */
export function clipPlaySec(clip: AudioClip, outputDuration: number): number {
	return clip.durationSec > 0
		? clip.durationSec
		: Math.max(0, outputDuration - clip.startOutputSec);
}

/** Clip end on the output axis. */
export function clipEndSec(clip: AudioClip, outputDuration: number): number {
	return clip.startOutputSec + clipPlaySec(clip, outputDuration);
}

/** Move a clip so its start lands at `newStart`, preserving its played length
 *  (a fill clip is materialized to a concrete duration) and staying in-bounds. */
export function moveClip(clip: AudioClip, newStart: number, outputDuration: number): AudioClip {
	const play = clipPlaySec(clip, outputDuration);
	const maxStart = Math.max(0, outputDuration - play);
	const start = Math.max(0, Math.min(newStart, maxStart));
	return { ...clip, startOutputSec: start, durationSec: play };
}

/** Drag the right edge to `newEnd` (start fixed). */
export function trimClipRight(clip: AudioClip, newEnd: number, outputDuration: number): AudioClip {
	const end = Math.max(clip.startOutputSec + MIN_CLIP_DURATION, Math.min(newEnd, outputDuration));
	return { ...clip, durationSec: end - clip.startOutputSec };
}

/** Drag the left edge to `newStart` (end fixed). Non-looping clips advance their
 *  source offset so the revealed audio is continuous; looping clips keep it. */
export function trimClipLeft(clip: AudioClip, newStart: number, outputDuration: number): AudioClip {
	const end = clipEndSec(clip, outputDuration);
	const start = Math.max(0, Math.min(newStart, end - MIN_CLIP_DURATION));
	const delta = start - clip.startOutputSec;
	const offsetSec = clip.loop ? clip.offsetSec : Math.max(0, clip.offsetSec + delta);
	return { ...clip, startOutputSec: start, durationSec: end - start, offsetSec };
}

/** Split a clip at output time `atOutputSec` into [left, right], or null if the
 *  cut isn't strictly inside the clip. The seam loses its fades (outer fades stay
 *  on the ends); a looping clip keeps its offset (the loop restarts, never a
 *  silent tail), a non-looping one advances the right half's offset to continue. */
export function splitClip(
	clip: AudioClip,
	atOutputSec: number,
	outputDuration: number,
	newId: string,
): [AudioClip, AudioClip] | null {
	const start = clip.startOutputSec;
	const end = clipEndSec(clip, outputDuration);
	if (atOutputSec <= start + MIN_CLIP_DURATION || atOutputSec >= end - MIN_CLIP_DURATION) {
		return null;
	}
	const leftPlay = atOutputSec - start;
	const left: AudioClip = { ...clip, durationSec: leftPlay, fadeOut: 0 };
	const right: AudioClip = {
		...clip,
		id: newId,
		startOutputSec: atOutputSec,
		durationSec: end - atOutputSec,
		offsetSec: clip.loop ? clip.offsetSec : clip.offsetSec + leftPlay,
		fadeIn: 0,
	};
	return [left, right];
}

// ---- Attribution / credits --------------------------------------------------

/** A credit line for a clip whose license requires attribution (CC-BY etc.). */
export interface MusicCredit {
	id: string;
	attribution: string;
	license: string | null;
}

/** Credits for every provider clip that carries attribution, deduped by line
 *  (the same track added twice is one credit). Local imports need no credit. */
export function collectCredits(clips: AudioClip[]): MusicCredit[] {
	const seen = new Set<string>();
	const out: MusicCredit[] = [];
	for (const clip of clips) {
		if (clip.source.kind !== "provider") continue;
		const attribution = clip.source.attribution?.trim();
		if (!attribution || seen.has(attribution)) continue;
		seen.add(attribution);
		out.push({ id: clip.id, attribution, license: clip.source.license?.trim() || null });
	}
	return out;
}

// ---- Provider seam (future third-party music APIs) --------------------------

export interface MusicSearchResult {
	trackId: string;
	title: string;
	artist?: string;
	durationSec?: number;
	attribution?: string;
	license?: string;
	/** Remote URL the provider downloads to cache the track offline. */
	downloadUrl: string;
	/** Streaming URL for auditioning before download (an <audio> src). */
	previewUrl?: string;
}

/**
 * A source of music tracks. A remote provider implements `search` + `resolve`,
 * where `resolve` downloads the track locally and returns a `provider` source
 * (so playback stays offline). `resolve` takes the full result so the provider
 * stays stateless between the two calls.
 */
export interface MusicProvider {
	id: string;
	label: string;
	search(query: string, page?: number): Promise<MusicSearchResult[]>;
	resolve(result: MusicSearchResult): Promise<AudioClipSourceProvider>;
}
