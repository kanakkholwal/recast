/**
 * Export job consumer — drives the offline renderer from a serialized
 * {@link ExportJob}. DOM-free, so it runs in the render worker; the producer
 * (build-export-job) owns all the DOM-bound asset prep.
 *
 * There is almost nothing left to rebuild here: the engine evaluates the scene
 * itself, so this is the job's assets wired to the renderer's inputs.
 */

import { videoEncodingConfigFor } from "./browser-export-plan";
import { closeJobBitmaps, type ExportJob } from "./export-job";
import { renderTimelineToVideo } from "./offscreen-export";

/** Runtime callbacks that can't cross into the job payload (they're live handles). */
export interface ExportRuntime {
	onProgress?: (fraction: number) => void;
	signal?: AbortSignal;
}

/** Consume a job → encoded mp4 bytes. Closes the bitmaps it owns when done. */
export async function runExportJob(
	job: ExportJob,
	runtime: ExportRuntime = {},
): Promise<Uint8Array> {
	try {
		return await renderTimelineToVideo({
			videoUrl: job.videoUrl,
			scene: job.scene,
			timeMap: job.timeMap,
			sourceWidth: job.sourceWidth,
			sourceHeight: job.sourceHeight,
			width: job.width,
			height: job.height,
			fps: job.fps,
			outputDurationSec: job.outputDurationSec,
			// Rebuilt here, not in the job: the branded MediaBunny `Quality` cannot
			// cross `postMessage`.
			encodingConfig: videoEncodingConfigFor(job.quality),
			assets: {
				backgroundImage: job.backgroundImage,
				cursorSprites: job.cursorSprites,
				cursorTrack: job.cursorTrack,
				captionTrack: job.captionTrack,
				captionFont: job.captionFont,
				annotationImages: job.annotationImages,
			},
			camera: job.camera,
			onProgress: runtime.onProgress,
			signal: runtime.signal,
		});
	} finally {
		// Every bitmap the job owns, deduped and idempotent, so an early throw
		// (engine init, say) cannot leak the sprites.
		closeJobBitmaps(job);
	}
}
