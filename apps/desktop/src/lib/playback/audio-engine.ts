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

import { AudioChunkStore } from "./audio-chunk-store";
import { gainFromPercent } from "./audio-gain";
import {
	outputToSource,
	planAudioScheduleWindow,
	type Region,
	sliceChunksForPlayback,
} from "./audio-schedule";

/**
 * Output-time position the listener is actually HEARING right now.
 *
 * `AudioContext.currentTime` is when a sample is handed to the hardware, not
 * when it reaches the ears — Bluetooth and USB interfaces add 100-300ms. Using
 * it raw as the master clock drags the picture that far ahead of the sound.
 * Clamped at the anchor: before playback is audible there is nothing to follow.
 */
export function heardOutputSec(
	anchorOutputTime: number,
	anchorCtxTime: number,
	ctxCurrentTime: number,
	outputLatencySec: number,
): number {
	const latency = Number.isFinite(outputLatencySec) ? Math.max(0, outputLatencySec) : 0;
	const elapsed = ctxCurrentTime - latency - anchorCtxTime;
	return anchorOutputTime + Math.max(0, elapsed);
}

/**
 * Fade envelope gain (0–1) at an OUTPUT-time position. Mirrors the export's
 * `afade` math (commands/editor.rs): each fade is clamped to half the output
 * duration, fade-in ramps 0→1 over [0, fadeIn], fade-out ramps 1→0 over
 * [outDur−fadeOut, outDur]. Kept pure so preview and export stay in lockstep.
 */
export function fadeGainAt(
	outSec: number,
	fadeIn: number,
	fadeOut: number,
	outDur: number,
): number {
	if (!(outDur > 0)) return 1;
	const inD = Math.max(0, Math.min(fadeIn, outDur * 0.5));
	const outD = Math.max(0, Math.min(fadeOut, outDur * 0.5));
	let g = 1;
	if (inD > 0 && outSec < inD) g = outSec / inD;
	const outStart = outDur - outD;
	if (outD > 0 && outSec > outStart) g = Math.min(g, (outDur - outSec) / outD);
	return Math.max(0, Math.min(1, g));
}

/**
 * Clip-fade factor (0–1) at `intoClip` seconds into a music clip's play window.
 * Mirrors the export's per-clip `afade` (fade clamped to the play length, not
 * half of it — unlike the master fade). Kept pure for testing + parity.
 */
export function musicFadeFactor(
	intoClip: number,
	play: number,
	fadeIn: number,
	fadeOut: number,
): number {
	if (!(play > 0)) return 1;
	const fi = Math.min(Math.max(0, fadeIn), play);
	const fo = Math.min(Math.max(0, fadeOut), play);
	let f = 1;
	if (fi > 0 && intoClip < fi) f = intoClip / fi;
	const foStart = play - fo;
	if (fo > 0 && intoClip > foStart) f = Math.min(f, (play - intoClip) / fo);
	return Math.max(0, Math.min(1, f));
}

/** A music/extra-audio clip resolved for playback (url already asset-resolved). */
export interface MusicClipSpec {
	url: string;
	startOutputSec: number;
	offsetSec: number;
	durationSec: number; // 0 = fill to output end
	gain: number; // 0–200
	fadeIn: number;
	fadeOut: number;
	loop: boolean;
}

export type AudioTrackKind = "system" | "mic";

export interface AudioTrack {
	store: AudioChunkStore;
	gain: GainNode;
	kind: AudioTrackKind;
}

export interface AudioTrackSpec {
	/** URL to fetch + decode. `null` skips this slot. */
	url: string | null;
	kind: AudioTrackKind;
}

// How far ahead of the playhead (output seconds) the streaming scheduler keeps
// audio scheduled + decoded, and how far behind it keeps decoded chunks before
// evicting. The window bounds resident PCM regardless of recording length.
const AUDIO_LOOKAHEAD_SEC = 12;
const AUDIO_BEHIND_SEC = 4;
const TOPUP_INTERVAL_MS = 2000;

export class AudioTimelineEngine {
	#ctx: AudioContext;
	#tracks: AudioTrack[] = [];
	#active: AudioBufferSourceNode[] = [];
	#volume = 1; // 0..1, master
	#muted = false;
	#trackVolumes: Record<AudioTrackKind, number> = { system: 1, mic: 1 };
	#trackMuted: Record<AudioTrackKind, boolean> = { system: false, mic: false };
	// Master fade-in/out envelope, applied downstream of the per-track gains so
	// the exported `afade` is audible in the preview too.
	#fadeGain: GainNode;
	#fadeIn = 0;
	#fadeOut = 0;
	#outputDuration = 0;
	// Anchor mapping output time onto the audio hardware clock, so the picture
	// can follow audio instead of free-running on a second, drifting clock.
	#anchorCtxTime = 0;
	#anchorOutputTime = 0;
	#scheduled = false;
	// Music/extra-audio clips on the output timeline, each with its own gain
	// (routed straight to destination — the master fade applies to the recording
	// only, matching the export where music is amixed AFTER the source's fade).
	#music: Array<{ spec: MusicClipSpec; buffer: AudioBuffer; gain: GainNode }> = [];
	#musicActive: AudioBufferSourceNode[] = [];
	// Streaming schedule-ahead state: schedule ~AUDIO_LOOKAHEAD_SEC of output
	// time past the playhead and evict decoded chunks behind it, so a long
	// recording never holds the whole file's PCM at once.
	#regions: ReadonlyArray<Region> = [];
	#scheduledUpToOutput = 0;
	#topUpTimer: ReturnType<typeof setInterval> | undefined;
	#topUpRunning = false;
	// Bumped on every (re)schedule so an in-flight async top-up bails instead
	// of scheduling stale nodes after a scrub.
	#generation = 0;
	// Aborts the in-flight decode on a scrub so a stale 12s window can't block the
	// fresh one — without it a seek can stall until the next interval top-up.
	#abort: AbortController | null = null;

	private constructor(ctx: AudioContext, tracks: AudioTrack[], fadeGain: GainNode) {
		this.#ctx = ctx;
		this.#tracks = tracks;
		this.#fadeGain = fadeGain;
	}

	/**
	 * Create the engine for the given audio source specs (system + mic, in
	 * any order; nulls skipped), decoding each into an `AudioBuffer`. Throws
	 * if Web Audio is unavailable or nothing decodes; caller falls back to the
	 * `<audio>` elements.
	 */
	static async create(specs: ReadonlyArray<AudioTrackSpec>): Promise<AudioTimelineEngine> {
		const Ctx: typeof AudioContext | undefined =
			typeof AudioContext !== "undefined"
				? AudioContext
				: // eslint-disable-next-line @typescript-eslint/no-explicit-any
					(globalThis as any).webkitAudioContext;
		if (!Ctx) throw new Error("Web Audio API unavailable");

		const ctx = new Ctx();
		// Per-track gains feed a shared fade node feeding the destination, so the
		// fade envelope rides the whole mix.
		const fadeGain = ctx.createGain();
		fadeGain.connect(ctx.destination);
		const tracks: AudioTrack[] = [];
		for (const spec of specs) {
			if (!spec.url) continue;
			try {
				const store = await AudioChunkStore.create(spec.url);
				if (!store) continue;
				const gain = ctx.createGain();
				gain.connect(fadeGain);
				tracks.push({ store, gain, kind: spec.kind });
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
			throw new Error("no decodable audio tracks");
		}
		return new AudioTimelineEngine(ctx, tracks, fadeGain);
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

	/** Apply the master volume (0–200) and mute flag. */
	setMasterVolume(volume0to100: number, muted: boolean): void {
		this.#volume = gainFromPercent(volume0to100);
		this.#muted = muted;
		this.#applyGains();
	}

	/**
	 * Apply volume (0–100) and mute to a single track (system or mic). The
	 * master volume still gates; calling this with `volume: 0` is the
	 * supported way to silence just the mic without affecting system audio.
	 */
	setTrackVolume(kind: AudioTrackKind, volume0to100: number, muted: boolean): void {
		this.#trackVolumes[kind] = gainFromPercent(volume0to100);
		this.#trackMuted[kind] = muted;
		this.#applyGains();
	}

	/**
	 * Set the fade-in/out envelope (seconds) and the total OUTPUT duration they
	 * ride on. Re-arms the envelope immediately if playback is scheduled.
	 */
	setFades(fadeIn: number, fadeOut: number, outputDuration: number): void {
		this.#fadeIn = Math.max(0, fadeIn);
		this.#fadeOut = Math.max(0, fadeOut);
		this.#outputDuration = Math.max(0, outputDuration);
		if (this.#scheduled) this.#scheduleFades(this.positionOutputSec ?? this.#anchorOutputTime);
	}

	// Schedule the fade envelope on the audio clock. Output time o maps to ctx
	// time now + (o − from), so ramp breakpoints land where the ear expects.
	#scheduleFades(from: number): void {
		const g = this.#fadeGain.gain;
		const now = this.#ctx.currentTime;
		g.cancelScheduledValues(now);
		const outDur = this.#outputDuration;
		const inD = Math.max(0, Math.min(this.#fadeIn, outDur * 0.5));
		const outD = Math.max(0, Math.min(this.#fadeOut, outDur * 0.5));
		if (!(outDur > 0) || (inD <= 0 && outD <= 0)) {
			g.setValueAtTime(1, now);
			return;
		}
		const ctxAt = (o: number) => now + Math.max(0, o - from);
		g.setValueAtTime(fadeGainAt(from, this.#fadeIn, this.#fadeOut, outDur), now);
		if (inD > 0 && from < inD) g.linearRampToValueAtTime(1, ctxAt(inD));
		if (outD > 0) {
			const outStart = outDur - outD;
			if (from < outStart) g.setValueAtTime(1, ctxAt(outStart));
			g.linearRampToValueAtTime(0, ctxAt(outDur));
		}
	}

	/**
	 * Replace the music/extra-audio clips. Decodes each `url` (skipping any that
	 * fail) and re-schedules if playback is live. Silent/zero-gain clips are
	 * dropped, mirroring the export.
	 */
	async setMusicClips(clips: ReadonlyArray<MusicClipSpec>): Promise<void> {
		this.#disposeMusic();
		for (const spec of clips) {
			if (spec.gain <= 0) continue;
			try {
				const res = await fetch(spec.url);
				if (!res.ok) continue;
				const buffer = await this.#ctx.decodeAudioData(await res.arrayBuffer());
				const gain = this.#ctx.createGain();
				gain.connect(this.#ctx.destination);
				this.#music.push({ spec, buffer, gain });
			} catch {
				// Skip a clip that won't fetch/decode.
			}
		}
		if (this.#scheduled) this.#scheduleMusic(this.positionOutputSec ?? this.#anchorOutputTime);
	}

	#scheduleMusic(from: number): void {
		this.#stopMusic();
		const now = this.#ctx.currentTime;
		const outDur = this.#outputDuration;
		for (const { spec, buffer, gain } of this.#music) {
			const play =
				spec.durationSec > 0 ? spec.durationSec : Math.max(0, outDur - spec.startOutputSec);
			if (play <= 0) continue;
			const clipEnd = spec.startOutputSec + play;
			if (from >= clipEnd) {
				gain.gain.value = 0;
				continue;
			}
			const heardStart = Math.max(spec.startOutputSec, from);
			const whenDelay = heardStart - from;
			const intoClip = heardStart - spec.startOutputSec;
			const remaining = clipEnd - heardStart;
			const bufDur = buffer.duration;
			const region = Math.max(0.001, bufDur - spec.offsetSec);
			let sourceStart: number;
			if (spec.loop) {
				sourceStart = spec.offsetSec + (intoClip % region);
			} else {
				sourceStart = spec.offsetSec + intoClip;
				if (sourceStart >= bufDur) {
					gain.gain.value = 0;
					continue;
				}
			}
			const node = this.#ctx.createBufferSource();
			node.buffer = buffer;
			if (spec.loop) {
				node.loop = true;
				node.loopStart = spec.offsetSec;
				node.loopEnd = bufDur;
			}
			node.connect(gain);
			const playDur = spec.loop ? remaining : Math.min(remaining, bufDur - sourceStart);
			node.onended = () => {
				const i = this.#musicActive.indexOf(node);
				if (i >= 0) this.#musicActive.splice(i, 1);
			};
			node.start(now + whenDelay, sourceStart, playDur);
			this.#musicActive.push(node);
			// Base gain + clip fades on the output clock.
			const base = Math.max(0, Math.min(2, spec.gain / 100));
			const g = gain.gain;
			const ctxAt = (o: number) => now + Math.max(0, o - from);
			g.cancelScheduledValues(now);
			g.setValueAtTime(base * musicFadeFactor(intoClip, play, spec.fadeIn, spec.fadeOut), now);
			if (spec.fadeIn > 0 && intoClip < Math.min(spec.fadeIn, play)) {
				g.linearRampToValueAtTime(base, ctxAt(spec.startOutputSec + Math.min(spec.fadeIn, play)));
			}
			if (spec.fadeOut > 0) {
				const foStart = clipEnd - Math.min(spec.fadeOut, play);
				if (heardStart < foStart) g.setValueAtTime(base, ctxAt(foStart));
				g.linearRampToValueAtTime(0, ctxAt(clipEnd));
			}
		}
	}

	#stopMusic(): void {
		for (const node of this.#musicActive) {
			try {
				node.onended = null;
				node.stop();
			} catch {
				/* already stopped */
			}
		}
		this.#musicActive = [];
	}

	#disposeMusic(): void {
		this.#stopMusic();
		for (const m of this.#music) {
			try {
				m.gain.disconnect();
			} catch {
				/* already gone */
			}
		}
		this.#music = [];
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

	/** (Re)schedule kept regions so audio plays from OUTPUT time `from`. Streams a
	 *  window ahead + evicts behind rather than materialising the whole file. */
	#schedule(regions: ReadonlyArray<Region>, from: number): void {
		this.#stopActive();
		this.#stopTopUp();
		// Cut short any decode from the previous position so this seek's window
		// starts immediately instead of queueing behind a now-stale 12s decode.
		this.#abort?.abort();
		this.#abort = new AbortController();
		this.#anchorCtxTime = this.#ctx.currentTime;
		this.#anchorOutputTime = from;
		this.#scheduled = true;
		this.#regions = regions;
		this.#scheduledUpToOutput = from;
		this.#generation++;
		this.#scheduleFades(from);
		this.#scheduleMusic(from);
		void this.#topUp();
		this.#topUpTimer = setInterval(() => void this.#topUp(), TOPUP_INTERVAL_MS);
	}

	// Schedule the next output slice a window ahead of the playhead and evict
	// decoded audio behind it. whenDelay is anchored to the fixed play-start, so
	// scheduling ahead still lands each source at the correct ctx time.
	async #topUp(): Promise<void> {
		if (!this.#scheduled || this.#topUpRunning || this.#tracks.length === 0) return;
		this.#topUpRunning = true;
		const gen = this.#generation;
		const signal = this.#abort?.signal;
		try {
			const heard = this.positionOutputSec ?? this.#anchorOutputTime;
			const windowEnd = heard + AUDIO_LOOKAHEAD_SEC;
			if (windowEnd > this.#scheduledUpToOutput + 1e-4) {
				const plan = planAudioScheduleWindow(
					this.#regions,
					this.#anchorOutputTime,
					this.#scheduledUpToOutput,
					windowEnd,
				);
				for (const track of this.#tracks) {
					for (const pc of plan) {
						await track.store.ensureRange(pc.bufferOffset, pc.bufferOffset + pc.duration, signal);
						// A scrub/dispose during decode invalidates this pass.
						if (this.#generation !== gen || !this.#scheduled) return;
						for (const sub of sliceChunksForPlayback(pc, track.store.chunks())) {
							const buf = track.store.buffer(sub.chunkIndex);
							if (!buf) continue;
							let startAt = this.#anchorCtxTime + sub.whenDelay;
							let offset = sub.offsetInChunk;
							let dur = sub.playDuration;
							// Late (decode slower than the lead): skip the past part so we stay
							// in sync instead of playing it delayed.
							const late = this.#ctx.currentTime - startAt;
							if (late > 0) {
								offset += late * sub.rate;
								dur -= late * sub.rate;
								startAt = this.#ctx.currentTime;
								if (dur <= 1e-4) continue;
							}
							const node = this.#ctx.createBufferSource();
							node.buffer = buf;
							node.playbackRate.value = sub.rate;
							node.connect(track.gain);
							node.onended = () => {
								const i = this.#active.indexOf(node);
								if (i >= 0) this.#active.splice(i, 1);
							};
							node.start(startAt, offset, dur);
							this.#active.push(node);
						}
					}
				}
				this.#scheduledUpToOutput = windowEnd;
			}
			const behind = outputToSource(this.#regions, Math.max(0, heard - AUDIO_BEHIND_SEC));
			for (const track of this.#tracks) track.store.evictBefore(behind);
		} catch (err) {
			console.error("audio streaming top-up failed:", err);
		} finally {
			this.#topUpRunning = false;
			// A scrub bumped the generation while we were decoding (and the guard above
			// skipped its own top-up). Re-run now for the current playhead instead of
			// waiting out the interval, so audio catches up right after the seek.
			if (this.#scheduled && this.#generation !== gen) void this.#topUp();
		}
	}

	#stopTopUp(): void {
		if (this.#topUpTimer !== undefined) {
			clearInterval(this.#topUpTimer);
			this.#topUpTimer = undefined;
		}
	}

	/** Start (or restart) playback from OUTPUT time `fromOutputTime`. */
	async play(regions: ReadonlyArray<Region>, fromOutputTime: number): Promise<void> {
		if (this.#ctx.state === "suspended") {
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
		this.#stopMusic();
		this.#stopTopUp();
		this.#abort?.abort();
		this.#scheduled = false;
	}

	/**
	 * Output-time position according to the audio hardware clock, or null when
	 * nothing is scheduled. Output time advances 1:1 with the audio clock —
	 * per-segment speed is folded into each chunk's offset, not its start time.
	 */
	get positionOutputSec(): number | null {
		if (!this.#scheduled) return null;
		return heardOutputSec(
			this.#anchorOutputTime,
			this.#anchorCtxTime,
			this.#ctx.currentTime,
			this.outputLatencySec,
		);
	}

	/**
	 * Hardware output latency in seconds. `outputLatency` is the accurate one
	 * but is newer; `baseLatency` (the graph's own buffering) is the fallback
	 * and undercounts, which is still better than assuming zero.
	 */
	get outputLatencySec(): number {
		const ctx = this.#ctx as AudioContext & { outputLatency?: number };
		if (typeof ctx.outputLatency === "number" && Number.isFinite(ctx.outputLatency)) {
			return ctx.outputLatency;
		}
		return Number.isFinite(ctx.baseLatency) ? ctx.baseLatency : 0;
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
		this.#stopTopUp();
		this.#abort?.abort();
		this.#disposeMusic();
		this.#scheduled = false;
		for (const t of this.#tracks) t.store.dispose();
		this.#tracks = [];
		try {
			void this.#ctx.close();
		} catch {
			/* already closed */
		}
	}
}
