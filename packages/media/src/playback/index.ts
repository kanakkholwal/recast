/**
 * Worker-bridged playback, on its own subpath so consumers that only want
 * conversion helpers don't pull the decode worker into their bundle.
 *
 * `MediabunnyVideoSource` is the real implementation of the playback surface
 * REQUIREMENTS.md §2 describes. It moved here from
 * `apps/desktop/src/lib/playback/mediabunny-source.ts` — it was always pure
 * Web-platform code (Worker + WebCodecs + OffscreenCanvas, no Tauri), so
 * living in the desktop app made it unusable from `apps/web` and left the
 * package exporting throw-stubs in its place.
 */

export { MediabunnyVideoSource } from './source';
export type { FromMediabunnyWorker, ToMediabunnyWorker } from './worker';
