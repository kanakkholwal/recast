/**
 * Music / extra-audio clips placed on the OUTPUT timeline (over the finished
 * edit), distinct from the recording's own system/mic tracks. v1 imports local
 * files; the `MusicProvider` interface is the seam a future licensed-music API
 * (Uppbeat / Pixabay / Epidemic) plugs into — a provider result is cached into
 * the `.recast` bundle and stored as a `provider` source, so playback stays
 * offline-first (never a hotlink URL).
 */

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

/** A piece of audio laid on the output timeline (background music, voiceover…). */
export interface AudioClip {
	id: string;
	source: AudioClipSource;
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

/** Open an audio file picker. Returns the absolute path, or null if cancelled. */
export async function pickAudioFile(): Promise<string | null> {
	const { open } = await import("@tauri-apps/plugin-dialog");
	const selected = await open({
		multiple: false,
		directory: false,
		title: "Add music or audio",
		filters: [{ name: "Audio", extensions: ["mp3", "wav", "m4a", "aac", "ogg", "flac"] }],
	});
	return typeof selected === "string" ? selected : null;
}

/** Display name for a clip (the source file's basename). */
export function clipDisplayName(clip: AudioClip): string {
	const p = clipAssetPath(clip.source);
	const base = p.split(/[\\/]/).pop() ?? p;
	return base || "Audio";
}

// ---- Provider seam (future third-party music APIs) --------------------------

export interface MusicSearchResult {
	trackId: string;
	title: string;
	durationSec?: number;
	attribution?: string;
}

/**
 * A source of music tracks. v1 ships only local-file import (no provider); a
 * remote provider implements `search` + `resolve`, where `resolve` downloads
 * the track into the project bundle and returns a `provider` source.
 */
export interface MusicProvider {
	id: string;
	label: string;
	search(query: string, page?: number): Promise<MusicSearchResult[]>;
	resolve(trackId: string): Promise<AudioClipSourceProvider>;
}
