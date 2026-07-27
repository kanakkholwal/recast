/**
 * Feature flags for in-progress / platform-gated functionality.
 *
 * These gate UI surfaces only. Capture/render/export stay wired up, so
 * flipping a flag back to `true` re-enables the feature without other changes.
 * See the matching design note under `apps/desktop/docs/`.
 */

/**
 * Editor-side camera overlay UI (properties tab + draggable preview overlay).
 * Enabled now that camera recording (WebView MediaRecorder), editor
 * resize/reposition, and zoom-follow with preview↔export parity are wired.
 */
export const CAMERA_OVERLAY_UI_ENABLED = true;
