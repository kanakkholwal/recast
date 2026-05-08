/**
 * Feature flags for in-progress / platform-gated functionality.
 *
 * These flags gate UI surfaces only — the underlying capture, render, and
 * export pipelines remain wired up. Flipping a flag back to `true` should
 * re-enable the feature without any code changes elsewhere.
 *
 * If you're touching one of these, also read the matching design note under
 * `apps/desktop/docs/`.
 */

/**
 * Editor-side camera overlay UI (properties panel tab + draggable overlay
 * on the preview canvas).
 *
 * Recording with the camera still works — the bubble is captured to a
 * separate track in the .recast bundle exactly as before. This flag only
 * hides the editor controls for re-positioning, mirroring, shape, size,
 * etc. Re-enable plan + per-platform exclusion APIs (the eventual fix for
 * the floating preview window leaking into screen capture) are documented
 * in `apps/desktop/docs/camera-recording-todo.md`.
 */
export const CAMERA_OVERLAY_UI_ENABLED = false;
