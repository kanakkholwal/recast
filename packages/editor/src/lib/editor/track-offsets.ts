/**
 * How far each companion capture track starts after video frame 0, measured at
 * record time by the Rust session. Every device comes up at its own instant, so
 * without these the preview and the export both stack the tracks at 0 and bake
 * the skew in. Mirrors `recording::TrackOffsets`.
 */
export interface TrackOffsets {
	audioMs: number;
	microphoneMs: number;
	cameraMs: number;
}

/** As it arrives over IPC: absent fields mean the track was never measured. */
export interface TrackOffsetsWire {
	audioMs?: number | null;
	microphoneMs?: number | null;
	cameraMs?: number | null;
}

export const ZERO_TRACK_OFFSETS: TrackOffsets = {
	audioMs: 0,
	microphoneMs: 0,
	cameraMs: 0,
};

/** Mirrors `MAX_CORRECTION_MS` in the Rust `export::align` module: past this a
 *  measurement is a fault, and applying it would wreck an otherwise fine take. */
export const MAX_OFFSET_MS = 30_000;

function clean(ms: number | null | undefined): number {
	if (typeof ms !== "number" || !Number.isFinite(ms)) return 0;
	return Math.abs(ms) > MAX_OFFSET_MS ? 0 : ms;
}

/**
 * Normalise wire offsets into a total record. Unmeasured, non-finite and
 * implausible values all collapse to 0, which is the pre-measurement behaviour.
 */
export function resolveTrackOffsets(wire: TrackOffsetsWire | null | undefined): TrackOffsets {
	if (!wire) return { ...ZERO_TRACK_OFFSETS };
	return {
		audioMs: clean(wire.audioMs),
		microphoneMs: clean(wire.microphoneMs),
		cameraMs: clean(wire.cameraMs),
	};
}

/**
 * Source time to read a companion track at, for a moment at `timelineSec` on
 * the video's timeline. Never negative: before the track existed there is
 * nothing to read, so callers get its first sample.
 */
export function trackTimeAt(timelineSec: number, offsetMs: number): number {
	return Math.max(0, timelineSec - offsetMs / 1000);
}
