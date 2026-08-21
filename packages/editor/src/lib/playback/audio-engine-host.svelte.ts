/**
 * Audio-engine lifecycle for a host that mounts `<Editor />`. The engine is
 * HOST-owned — an `AudioContext` is an OS audio thread plus decoded PCM, and a
 * host that also drives its own transport must not race a second engine the
 * editor built behind its back.
 *
 * Call during component init; the returned holder disposes on teardown.
 */

import { untrack } from "svelte";
import { AudioTimelineEngine, type AudioTrackSpec } from "./audio-engine";

export interface AudioEngineHolder {
	readonly current: AudioTimelineEngine | null;
}

/**
 * Rebuilds whenever `specs()` changes. Adopting a stale engine would strand its
 * `AudioContext` and PCM, so the previous one is disposed before the swap and a
 * generation counter drops any create that resolves after it was superseded.
 */
export function createAudioEngineHost(
	specs: () => readonly AudioTrackSpec[] | undefined,
): AudioEngineHolder {
	let engine = $state<AudioTimelineEngine | null>(null);
	let generation = 0;

	$effect(() => {
		const next = specs();
		const gen = ++generation;
		untrack(() => engine)?.dispose();
		engine = null;
		if (!next?.length) return;
		void AudioTimelineEngine.create([...next])
			.then((created) => {
				if (gen !== generation) {
					created.dispose();
					return;
				}
				engine = created;
			})
			.catch(() => {
				// Nothing decodable: the preview stays silent rather than failing.
			});
		return () => {
			generation++;
			untrack(() => engine)?.dispose();
			engine = null;
		};
	});

	return {
		get current() {
			return engine;
		},
	};
}
