/**
 * App-side entry to the shared decode engine. The package drives the worker
 * but never spawns it — see `MediabunnySourceOptions` — so the spawn lives
 * here, next to the worker entry it points at.
 */

import { type MediaRef, MediabunnyVideoSource } from "@recast/media/playback";

/**
 * `known` is the recording's ffprobe metadata. Passing it lets the worker skip
 * `computeDuration()` and `computePacketStats()`, both of which walk the whole
 * container on a fragmented MP4 — that was the 30s open timeout on 4K files.
 */
export function createMediabunnySource(
	src: MediaRef | Blob | string,
	known?: { durationSec?: number; fps?: number },
): Promise<MediabunnyVideoSource> {
	return MediabunnyVideoSource.create(src, {
		createWorker: () =>
			new Worker(new URL("./mediabunny-worker", import.meta.url), { type: "module" }),
		durationSec: known?.durationSec,
		fps: known?.fps,
	});
}
