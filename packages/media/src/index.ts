/**
 * Public API barrel for `@recast/media`. The ONLY entry point consumers
 * should import from. Direct imports from `mediabunny` are forbidden outside
 * this package (enforced by Biome lint + a CI grep check — see
 * packages/media/REQUIREMENTS.md §5).
 *
 * Consumers:
 * - `apps/desktop/src/routes/editor/[file]/` — preview pipeline (PR-D onwards).
 * - `apps/web/src/routes/tools/*` — conversion tools (PR-B onwards).
 * - Future: an in-browser editor for 100 GB+ source files.
 */

export type { ContainerKind, ConversionParams } from './conversion';
export {
	inputContainerKind,
	outputFormatFor,
	runConversion,
	withExtension,
} from './conversion';
export type { GifWriter } from './encoders';
// small encoders (PR-B)
export {
	createGifWriter,
	encodeMp3,
	encodeWav,
	zipFiles,
} from './encoders';
export type { MediaErrorCode } from './errors';
export { MediaError } from './errors';
export { handlers } from './handlers';
export type { MediaSource } from './input';
// input + conversion
export { openInput } from './input';
export type { PlaybackEvent, PlaybackFrame, PlaybackSource } from './playback';
// playback (worker-bridged; PR-D lands the implementation)
export {
	cacheStats,
	evictCache,
	openMediaSource,
	prefetchAround,
	seekTo,
} from './playback';
export type {
	ConvertErrorCode,
	ConvertHandler,
	ConvertJob,
	FromConvertWorker,
	HandlerResult,
	JobContext,
	ToConvertWorker,
	ToolOp,
	ToolOptions,
} from './protocol';
// conversion protocol + handlers (apps/web conversion tools)
export { ConvertError } from './protocol';
// seek helpers (PR-D)
export { nextCutWithin, snapToSeekTarget } from './seek';
// sources (PR-A: stub; PR-D lands encodeCanvasToMp4 if needed)
export { encodeCanvasToMp4 } from './sources';
// MediaBunny primitives re-exported so worker code living outside
// `packages/media` (e.g. `apps/desktop/src/lib/playback/mediabunny-worker.ts`)
// can compose them through this package without a direct mediabunny import.
// Worker code is the only allowed outside consumer; biome's
// `noRestrictedImports` allows it via a scoped override.
export { ALL_FORMATS, CanvasSink, Input, UrlSource } from 'mediabunny';
export type { InputVideoTrack, WrappedCanvas } from 'mediabunny';
