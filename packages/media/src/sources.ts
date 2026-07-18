/**
 * Media-source helpers — CanvasSource / VideoSampleSource wrappers and
 * re-exports. (PR-A: stub only; PR-D lands the real worker-bridged
 * PlaybackSource here.)
 *
 * The actual screenshot-editor animation export and any other use of
 * MediaBunny's `Output` / `CanvasSource` goes through the high-level
 * functions exported from this file. Direct `mediabunny` imports are
 * forbidden in consumer code (REQUIREMENTS.md §5).
 */

/**
 * Encode frames from a `<canvas>` to a media file using MediaBunny's
 * `CanvasSource`. The source reads the canvas on each `add()` call, so the
 * caller must paint into `canvas` before invoking `add`.
 *
 * PR-A: stub. The screenshot-editor's existing
 * `packages/application/src/screenshot-editor/video.ts` keeps its
 * `mp4-muxer` path; this helper exists for future direct-callers only.
 */
export async function encodeCanvasToMp4(
	_canvas: HTMLCanvasElement | OffscreenCanvas,
	_options: {
		fps: number;
		durationMs: number;
		onProgress?: (progress: number) => void;
	},
): Promise<Blob> {
	throw new Error('encodeCanvasToMp4 is not yet implemented — lands in a later PR');
}
