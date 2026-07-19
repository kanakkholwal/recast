/**
 * Feature flag for the MediaBunny-backed video preview path.
 *
 * Default ON (PR-F): the editor's preview now uses `MediabunnyVideoSource`,
 * which gives frame-accurate decode, sample-accurate cuts, and removes the
 * legacy `mp4box` + hand-rolled WebCodecs dependency. The legacy
 * `WebCodecsVideoSource` is still available for environments that can't
 * run MediaBunny (e.g. node tests, very old webviews); it's opt-in via
 * `?useLegacyPreview=1`.
 *
 * URLs:
 *   recast://editor/foo.recast                      → MediaBunny (default)
 *   recast://editor/foo.recast?useLegacyPreview=1   → WebCodecs + mp4box
 *   recast://editor/foo.recast?mbPreview=0          → legacy (alias of the above)
 *
 * The flag is loose on purpose; the only normalized value is the toggle
 * value, anything else defaults ON.
 */
export function isMediabunnyPreviewEnabled(): boolean {
	// Default: MediaBunny. The legacy pipeline stays as an opt-in fallback
	// (until PR-F deletes it entirely).
	if (typeof window === 'undefined') return true;
	const search = window.location?.search ?? '';
	if (!search) return true;
	const params = new URLSearchParams(search);
	// `useLegacyPreview=1` opts the editor back into the WebCodecs engine.
	const legacy = params.get('useLegacyPreview');
	if (legacy !== null) {
		return !isTruthy(legacy);
	}
	// `mbPreview=0` is the legacy alias; any other value is the new default.
	const mb = params.get('mbPreview');
	if (mb !== null) {
		return !isFalsy(mb);
	}
	return true;
}

function isTruthy(value: string): boolean {
	const v = value.trim().toLowerCase();
	return v === '1' || v === 'true' || v === 'yes' || v === 'on';
}

function isFalsy(value: string): boolean {
	const v = value.trim().toLowerCase();
	return v === '0' || v === 'false' || v === 'no' || v === 'off' || v === '';
}

/**
 * Human-readable label for telemetry / logs. Stable across releases so
 * the analytics event name doesn't drift.
 */
export const MEDIABUNNY_PREVIEW_FLAG = 'mbPreview';
export const LEGACY_PREVIEW_FLAG = 'useLegacyPreview';