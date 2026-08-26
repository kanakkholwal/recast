/**
 * Re-export shim. The scheduling math lives in `@recast/media` so the desktop
 * engine and any future web editor share one implementation — this file held a
 * byte-identical copy, which would have drifted on the first bug fix.
 */

export type { AudioChunk, Region, ScheduledChunk, SubPlay } from "@recast/media";
export {
	keptRegions,
	missingRanges,
	outputToSource,
	planAudioSchedule,
	planAudioScheduleWindow,
	sliceChunksForPlayback,
} from "@recast/media";
