/**
 * App-side entry to the shared decode engine. The package drives the worker
 * but never spawns it — see `MediabunnySourceOptions` — so the spawn lives
 * here, next to the worker entry it points at.
 */

import { type MediaRef, MediabunnyVideoSource } from "@recast/media/playback";
import { createEditorWorker } from "../host-hooks";

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
		createWorker: () => createEditorWorker("mediabunny"),
		durationSec: known?.durationSec,
		fps: known?.fps,
	});
}
