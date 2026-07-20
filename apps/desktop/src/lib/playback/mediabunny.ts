/**
 * App-side entry to the shared decode engine. The package drives the worker
 * but never spawns it — see `MediabunnySourceOptions` — so the spawn lives
 * here, next to the worker entry it points at.
 */

import { MediabunnyVideoSource } from '@recast/media/playback';

export function createMediabunnySource(url: string): Promise<MediabunnyVideoSource> {
	return MediabunnyVideoSource.create(url, {
		createWorker: () =>
			new Worker(new URL('./mediabunny-worker.ts', import.meta.url), { type: 'module' }),
	});
}
