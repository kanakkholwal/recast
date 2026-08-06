/**
 * Feature flags for in-progress / platform-gated functionality.
 *
 * These gate UI surfaces only. Capture/render/export stay wired up, so
 * flipping a flag back to `true` re-enables the feature without other changes.
 * See the matching design note under `apps/desktop/docs/`.
 */

/**
 * Migration master flag: run exports through the browser compositor (RenderCore,
 * WYSIWYG with the preview) + FFmpeg mux, instead of the legacy Rust/FFmpeg
 * compositor. OFF until the A/B parity gate passes; the resolver
 * (choose-export-engine.ts) still falls back to Rust per capability/eligibility.
 */
export const BROWSER_EXPORT_ENABLED = false;
