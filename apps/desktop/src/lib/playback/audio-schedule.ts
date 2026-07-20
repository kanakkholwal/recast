/**
 * Re-export shim. The scheduling math lives in `@recast/media` so the desktop
 * engine and any future web editor share one implementation — this file held a
 * byte-identical copy, which would have drifted on the first bug fix.
 */

export { keptRegions, planAudioSchedule } from "@recast/media";
export type { Region, ScheduledChunk } from "@recast/media";
