/**
 * Feature flag for the MediaBunny-backed video preview path.
 *
 * Default OFF: the existing `WebCodecsVideoSource` continues to serve the
 * editor's preview. The flag is an explicit opt-in so reviewers and
 * developers can A/B test the new pipeline without disturbing the rest of
 * the editor.
 *
 * Activated by appending `?mbPreview=1` to the editor route URL, e.g.
 * `recast://editor/foo.recast?mbPreview=1` (Tauri) or
 * `http://localhost:1420/editor/foo.recast?mbPreview=1` (vite dev).
 *
 * Anything else (no param, `0`, `false`, `no`, absent) → OFF. The string
 * check is loose on purpose; this is a developer-facing toggle.
 */
export function isMediabunnyPreviewEnabled(): boolean {
	if (typeof window === 'undefined') return false;
	const search = window.location?.search ?? '';
	if (!search) return false;
	const params = new URLSearchParams(search);
	const raw = params.get('mbPreview');
	if (raw === null) return false;
	const normalized = raw.trim().toLowerCase();
	return normalized === '1' || normalized === 'true' || normalized === 'yes' || normalized === 'on';
}

/**
 * Human-readable label for telemetry / logs. Stable across releases so
 * the analytics event name doesn't drift.
 */
export const MEDIABUNNY_PREVIEW_FLAG = 'mbPreview';
