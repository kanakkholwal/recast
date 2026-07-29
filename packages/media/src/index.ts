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
} from './playback';
export type { PlaybackSource, PlaybackFrame, PlaybackEvent } from './playback';
export { keptRegions, missingRanges, outputToSource, planAudioSchedule, planAudioScheduleWindow, sliceChunksForPlayback } from './audio/schedule';
export type { AudioChunk, Region, ScheduledChunk, SubPlay } from './audio/schedule';

// seek helpers
export { snapToSeekTarget, nextCutWithin } from './seek';

// sources

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
export { frameBudget, frameCacheCapBytes, textureRingFrames } from './cache/frame-budget';
export type { FrameBudget } from './cache/frame-budget';
export { estimateFrameBytes } from './cache/storage';
export type { CacheableFrame, FrameStorage } from './cache/storage';

// Formats MediaBunny cannot decode; the preview falls back to <video>.
export {
	UNSUPPORTED_FORMATS,
	isUnsupportedContainer,
	isUnsupportedCodec,
} from './cache/unsupported-formats';
export type { UnsupportedFormat } from './cache/unsupported-formats';

// MediaBunny primitives live on `@recast/media/mediabunny`. Re-exporting them
// here would pull the whole library into every consumer (REQUIREMENTS.md §3).
