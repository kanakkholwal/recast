/**
 * Web Audio timeline engine: sample-accurate, cut-aware audio playback for the
 * WebCodecs editor preview.
 *
 * Instead of seeking an `<audio>` element to the playhead (drifts across cuts,
 * stalls on cold starts, can cut out), we decode the audio once into
 * `AudioBuffer`s and schedule each KEPT region as its own `AudioBufferSourceNode`
 * on the audio hardware clock; the cuts are the gaps between chunks, silent and
 * exact, with no seeking during playback.
 *
 * Lifecycle mirrors the picture clock: `play`/`pause`/`reschedule`. Fallback-safe:
 * `create` throws if Web Audio is unavailable or nothing decodes, and the caller
 * drops back to the `<audio>`-element path.
 *
 * Per-track volume: each track carries a `kind` (system or mic) and its own
 * `GainNode`. The engine exposes `setMasterVolume(volume, muted)` for the
 * master mix and `setTrackVolume(kind, volume, muted)` for per-track control,
 * so the user can mute just the mic while keeping system audio loud (or
 * vice versa). Master mutes short-circuit the per-track gains so a user
 * pressing M to mute everything still works instantly.
 */

import { planAudioSchedule, type Region } from './audio-schedule';

export type AudioTrackKind = 'system' | 'mic';

export interface AudioTrack {
	buffer: AudioBuffer;
	gain: GainNode;
	kind: AudioTrackKind;
}

export interface AudioTrackSpec {
	/** URL to fetch + decode. `null` skips this slot. */
	url: string | null;
	kind: AudioTrackKind;
}

export class AudioTimelineEngine {
	#ctx: AudioContext;
	#tracks: AudioTrack[] = [];
	#active: AudioBufferSourceNode[] = [];
	#volume = 1; // 0..1, master
	#muted = false;
	#trackVolumes: Record<AudioTrackKind, number> = { system: 1, mic: 1 };
	#trackMuted: Record<AudioTrackKind, boolean> = { system: false, mic: false };
	// Anchor mapping output time onto the audio hardware clock, so the picture
	// can follow audio instead of free-running on a second, drifting clock.
	#anchorCtxTime = 0;
	#anchorOutputTime = 0;
	#scheduled = false;

	private constructor(ctx: AudioContext, tracks: AudioTrack[]) {
		this.#ctx = ctx;
		this.#tracks = tracks;
	}

	/**
	 * Create the engine for the given audio source specs (system + mic, in
	 * any order; nulls skipped), decoding each into an `AudioBuffer`. Throws
	 * if Web Audio is unavailable or nothing decodes; caller falls back to the
	 * `<audio>` elements.
	 */
	static async create(specs: ReadonlyArray<AudioTrackSpec>): Promise<AudioTimelineEngine> {
		const Ctx: typeof AudioContext | undefined =
			typeof AudioContext !== 'undefined'
				? AudioContext
				: // eslint-disable-next-line @typescript-eslint/no-explicit-any
					(globalThis as any).webkitAudioContext;
		if (!Ctx) throw new Error('Web Audio API unavailable');

		const ctx = new Ctx();
		const tracks: AudioTrack[] = [];
		for (const spec of specs) {
			if (!spec.url) continue;
			try {
				const res = await fetch(spec.url);
				if (!res.ok) continue;
				const data = await res.arrayBuffer();
				const buffer = await ctx.decodeAudioData(data);
				const gain = ctx.createGain();
				gain.connect(ctx.destination);
				tracks.push({ buffer, gain, kind: spec.kind });
			} catch {
				// Skip a track that won't fetch/decode; others may still work.
			}
		}
		if (tracks.length === 0) {
			try {
				await ctx.close();
			} catch {
				/* ignore */
			}
			throw new Error('no decodable audio tracks');
		}
		return new AudioTimelineEngine(ctx, tracks);
	}

	get ready(): boolean {
		return this.#tracks.length > 0;
	}

	/** Apply master volume (0–1) and mute to every track's gain. Per-track
	 *  volumes are layered on top so a user can keep the system loud and
	 *  mute only the mic. */
	#applyGains(): void {
		for (const t of this.#tracks) {
			// Master mute short-circuits; otherwise master × per-track gain.
			const muted = this.#muted || this.#trackMuted[t.kind];
			t.gain.gain.value = muted ? 0 : this.#volume * this.#trackVolumes[t.kind];
		}
	}

	/** Apply the master volume (0–100) and mute flag. */
	setMasterVolume(volume0to100: number, muted: boolean): void {
		this.#volume = Math.max(0, Math.min(1, volume0to100 / 100));
		this.#muted = muted;
		this.#applyGains();
	}

	/**
	 * Apply volume (0–100) and mute to a single track (system or mic). The
	 * master volume still gates; calling this with `volume: 0` is the
	 * supported way to silence just the mic without affecting system audio.
	 */
	setTrackVolume(kind: AudioTrackKind, volume0to100: number, muted: boolean): void {
		this.#trackVolumes[kind] = Math.max(0, Math.min(1, volume0to100 / 100));
		this.#trackMuted[kind] = muted;
		this.#applyGains();
	}

	#stopActive(): void {
		for (const node of this.#active) {
			try {
				node.onended = null;
				node.stop();
			} catch {
				/* already stopped */
			}
		}
		this.#active = [];
	}

	/** (Re)schedule all kept regions so audio plays from OUTPUT time `from`. */
	#schedule(regions: ReadonlyArray<Region>, from: number): void {
		this.#stopActive();
		const now = this.#ctx.currentTime;
		this.#anchorCtxTime = now;
		this.#anchorOutputTime = from;
		this.#scheduled = true;
		const chunks = planAudioSchedule(regions, from);
		for (const t of this.#tracks) {
			const bufDur = t.buffer.duration;
			for (const c of chunks) {
				// A track may be shorter than the timeline (e.g. mic stopped early);
				// clamp the slice to the available buffer (in SOURCE seconds).
				if (c.bufferOffset >= bufDur) continue;
				const playDur = Math.min(c.duration, bufDur - c.bufferOffset);
				if (playDur <= 0) continue;
				const node = this.#ctx.createBufferSource();
				node.buffer = t.buffer;
				// Per-segment speed: play the slice faster/slower (pitch shifts,
				// matches the sped-up video; pitch-preserved stretch is a follow-up).
				node.playbackRate.value = c.rate;
				node.connect(t.gain);
				node.onended = () => {
					const i = this.#active.indexOf(node);
					if (i >= 0) this.#active.splice(i, 1);
				};
				node.start(now + c.whenDelay, c.bufferOffset, playDur);
				this.#active.push(node);
			}
		}
	}

	/** Start (or restart) playback from OUTPUT time `fromOutputTime`. */
	async play(regions: ReadonlyArray<Region>, fromOutputTime: number): Promise<void> {
		if (this.#ctx.state === 'suspended') {
			try {
				await this.#ctx.resume();
			} catch {
				/* resume may reject if not yet user-activated; schedule anyway */
			}
		}
		this.#schedule(regions, fromOutputTime);
	}

	/** Stop all sound; keep buffers for the next play. */
	pause(): void {
		this.#stopActive();
		this.#scheduled = false;
	}

	/**
	 * Output-time position according to the audio hardware clock, or null when
	 * nothing is scheduled. Output time advances 1:1 with the audio clock —
	 * per-segment speed is folded into each chunk's offset, not its start time.
	 */
	get positionOutputSec(): number | null {
		if (!this.#scheduled) return null;
		return this.#anchorOutputTime + (this.#ctx.currentTime - this.#anchorCtxTime);
	}

	/**
	 * Re-plan playback to a new OUTPUT time: on a scrub, or when the cut set
	 * changes while playing. No-op while paused (the next `play` will schedule).
	 */
	reschedule(regions: ReadonlyArray<Region>, fromOutputTime: number): void {
		this.#schedule(regions, fromOutputTime);
	}

	dispose(): void {
		this.#stopActive();
		this.#scheduled = false;
		this.#tracks = [];
		try {
			void this.#ctx.close();
		} catch {
			/* already closed */
		}
	}
}
