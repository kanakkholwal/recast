/**
 * Formats MediaBunny cannot decode (or that we know are unsupported in
 * the Tauri webview's WebCodecs implementation that backs MediaBunny).
 *
 * This list exists so:
 *   1. `VideoPreview.svelte` can log a clear telemetry event when a
 *      user's input falls back to the `<video>` element because of these.
 *   2. The `mediabunny_preview_fallback` analytics event can include a
 *      coarse "format" tag without leaking PII.
 *   3. Tests can pin the set so a future MediaBunny upgrade that adds a
 *      container/codec shrinks the gap.
 *
 * Sources of truth (verified against mediabunny@1.49.0 in
 * `packages/media/node_modules/mediabunny/README.md`):
 *   - Containers MediaBunny supports: MP4, MOV, WebM, MKV, HLS, WAVE, MP3,
 *     Ogg, ADTS, FLAC, MPEG-TS.
 *   - Codecs MediaBunny supports (via WebCodecs): AVC/H.264, HEVC/H.265,
 *     VP8, VP9, AV1, AAC, MP3, Opus, FLAC, Vorbis, ALAC, PCM (any rate).
 *
 * "Things NEITHER can decode" — i.e. containers / codecs that MediaBunny
 * doesn't list AND the legacy webcodecs+mp4box path also didn't support.
 * For these inputs the editor preview falls back to the `<video>` element.
 */

export interface UnsupportedFormat {
	/** Container extension(s), lowercased, no dot. */
	readonly container: readonly string[];
	/** Codec identifier (for codec-level filters), or null for whole-container. */
	readonly codec: string | null;
	/** Short human description used in telemetry + tests. */
	readonly reason: string;
}

/**
 * Curated list of formats neither MediaBunny nor the legacy webcodecs
 * pipeline can decode. The desktop preview falls back to the `<video>`
 * element when the input matches any container here OR (codec !== null)
 * when the codec filter is reported as unsupported.
 *
 * "Things NEITHER can decode" — keep this list in sync with the actual
 * capability gap. Pinning it as a vitest fixture (see
 * `apps/desktop/src/lib/playback/__tests__/unsupported-formats.test.ts`
 * and `packages/media/test/unsupported-formats.test.ts`) catches drift.
 */
export const UNSUPPORTED_FORMATS: readonly UnsupportedFormat[] = [
	{
		container: ["avi"],
		codec: null,
		reason: "AVI container is not in MediaBunny's input format set",
	},
	{
		container: ["flv"],
		codec: null,
		reason: "Flash Video (FLV) container is not in MediaBunny's input format set",
	},
	{
		container: ["wmv", "asf"],
		codec: "vc-1",
		reason: "Windows Media Video (VC-1) is not in WebCodecs or MediaBunny",
	},
	{
		container: ["rm", "rmvb"],
		codec: "realvideo",
		reason: "RealVideo is not in WebCodecs or MediaBunny",
	},
	{
		container: ["3gp", "3g2"],
		codec: null,
		reason: "3GP/3G2 (mobile video) container is not in MediaBunny's input format set",
	},
];

/**
 * Check whether a given file extension or codec tag falls into the
 * "unsupported" gap. The desktop preview uses this to decide whether
 * the fallback telemetry event should fire and to include a coarse
 * format tag.
 */
export function isUnsupportedContainer(extension: string): boolean {
	const ext = extension.toLowerCase().replace(/^\./, "");
	return UNSUPPORTED_FORMATS.some((f) => f.container.includes(ext));
}

export function isUnsupportedCodec(codec: string): boolean {
	const c = codec.toLowerCase();
	return UNSUPPORTED_FORMATS.some((f) => f.codec !== null && f.codec.toLowerCase() === c);
}
