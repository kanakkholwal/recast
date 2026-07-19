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

export { MediaError } from './errors';
export type { MediaErrorCode } from './errors';

// input + conversion
export { openInput } from './input';
export type { MediaSource } from './input';
export {
	runConversion,
	outputFormatFor,
	inputContainerKind,
	withExtension,
} from './conversion';
export type { ContainerKind, ConversionParams } from './conversion';

// conversion protocol + handlers (apps/web conversion tools)
export { ConvertError } from './protocol';
export { handlers } from './handlers';
export type {
	ToolOp,
	ToolOptions,
	ConvertJob,
	JobContext,
	ConvertErrorCode,
	HandlerResult,
	ConvertHandler,
	ToConvertWorker,
	FromConvertWorker,
} from './protocol';

// playback (worker-bridged)
export {
	cacheStats,
	evictCache,
	openMediaSource,
	prefetchAround,
	seekTo,
	createAudioScheduler,
} from './playback';
export type { PlaybackSource, PlaybackFrame, PlaybackEvent } from './playback';
export type { AudioScheduler, AudioSchedulerConfig } from './playback';
export type { Region, ScheduledChunk } from './audio/schedule';

// seek helpers
export { snapToSeekTarget, nextCutWithin } from './seek';

// sources
export { encodeCanvasToMp4 } from './sources';

// small encoders
export {
	createGifWriter,
	encodeWav,
	encodeMp3,
	zipFiles,
} from './encoders';
export type { GifWriter } from './encoders';

// decoded-frame cache
export {
	FrameCache,
	getFrameCache,
	setFrameCache,
	setFrameStorage,
	resetFrameCache,
} from './cache';
export type { CacheStats, FrameCacheConfig } from './cache';
export { IndexedDBFrameStorage } from './cache/indexeddb-storage';
export { estimateFrameBytes } from './cache/storage';
export type { CacheableFrame, FrameStorage } from './cache/storage';

// MediaBunny primitives re-exported so worker code living outside
// `packages/media` can compose them through this package without a direct
// mediabunny import. Worker code is the only allowed outside consumer;
// biome's `noRestrictedImports` allows it via a scoped override.
export { ALL_FORMATS, CanvasSink, Input, UrlSource } from 'mediabunny';
export type { InputVideoTrack, WrappedCanvas } from 'mediabunny';