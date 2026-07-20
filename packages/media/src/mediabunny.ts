/**
 * Raw MediaBunny primitives, re-exported on a dedicated subpath.
 *
 * These used to live on the main barrel, which meant every consumer of
 * `@recast/media` — including ones that only wanted `formatBytes`-tier
 * helpers — statically pulled all of MediaBunny into their bundle, defeating
 * the tree-shaking rule and the 80 KB desktop budget in REQUIREMENTS.md §3.
 *
 * Worker modules outside `packages/media` are the only sanctioned consumers:
 * a worker bundled through Vite's `new URL(...)` form cannot resolve a class
 * re-exported through a barrel, so it needs a direct, narrow entry point.
 * Application code must keep using the high-level API on the main barrel.
 */

// biome-ignore-all lint/style/noRestrictedImports: this file IS the sanctioned
// re-export point for MediaBunny primitives (REQUIREMENTS.md §5).
export { ALL_FORMATS, BlobSource, CanvasSink, Input, UrlSource } from 'mediabunny';
export type { InputVideoTrack, WrappedCanvas } from 'mediabunny';
