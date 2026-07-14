/**
 * Feature flags for in-progress / platform-gated functionality.
 *
 * These gate UI surfaces only. Capture/render/export stay wired up, so
 * flipping a flag back to `true` re-enables the feature without other changes.
 * See the matching design note under `apps/desktop/docs/`.
 */

/**
 * Editor-side camera overlay UI (properties tab + draggable preview overlay).
 * Recording with the camera still works; this only hides the editor controls.
 * Re-enable plan + per-platform exclusion APIs are documented in
 * `apps/desktop/docs/camera-recording-todo.md`.
 */
export const CAMERA_OVERLAY_UI_ENABLED = false;
