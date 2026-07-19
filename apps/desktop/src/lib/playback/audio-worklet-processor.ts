/**
 * AudioWorklet processor: sample-accurate audio scheduling on the audio
 * render thread. The main-thread `AudioScheduler` posts scheduling commands
 * via MessagePort; the processor manages `AudioBufferSourceNode`s and a
 * `GainNode` here, where they're tied to the audio clock and immune to main-
 * thread jank.
 *
 * Why an AudioWorklet instead of `AudioContext.currentTime`-based scheduling
 * on the main thread (the legacy `audio-engine.ts` path): when the main
 * thread is busy with a frame decode, a layout, or a sync XHR, scheduling
 * commands pile up and audio drifts. The audio thread runs at its own
 * cadence and doesn't compete for CPU with the page.
 *
 * Wire format (main → worklet):
 *   { type: 'init', sampleRate, channels: Float32Array[], mute, volume }
 *     — replaces the current schedule with these channels + params.
 *   { type: 'schedule', chunks: [{ whenDelay, bufferOffset, duration, rate, ... }] }
 *     — atomically swaps the live source list. Used after `init` to lay
 *       the chunks on the audio clock.
 *   { type: 'volume', volume, mute }
 *     — updates gain without disturbing the source list.
 *   { type: 'pause' }
 *     — stops every active source; keeps the buffer around for the next
 *       `schedule`.
 *   { type: 'dispose' }
 *     — closes the AudioContext gain; the processor stops responding.
 *
 * Each message is processed in order on the audio thread; there's no
 * ack/cancel dance. Cancellation is "stop all current sources, schedule
 * the new chunks" — the cost of an extra `stop()` is negligible.
 */

// The processor runs in `AudioWorkletGlobalScope` (no `window`, no DOM).
// `currentTime` and `currentFrame` are scoped to this thread.
declare const sampleRate: number;
declare const currentTime: number;
declare const currentFrame: number;
declare class AudioWorkletProcessor {
	readonly port: MessagePort;
	constructor();
	process(): boolean;
}
declare function registerProcessor(name: string, processor: typeof AudioWorkletProcessor): void;

interface ScheduleMsg {
	type: 'schedule';
	chunks: Array<{
		/** Output-seconds from `now` to begin this chunk. */
		whenDelay: number;
		/** Offset (seconds) into the buffer to start playing from. */
		bufferOffset: number;
		/** Duration (SOURCE seconds) of the buffer to play. */
		duration: number;
		/** Playback rate (= region speed). 1 = normal. */
		rate: number;
	}>;
}

interface InitMsg {
	type: 'init';
	sampleRate: number;
	// Channels arrive via `postMessage`'s transfer list; the typed channel
	// data uses `Float32Array<ArrayBufferLike>` (TypeScript 5.7+ types
	// `Float32Array` as generic over the underlying buffer). `copyToChannel`
	// accepts this fine at runtime.
	channels: Float32Array<ArrayBufferLike>[];
	mute: boolean;
	volume: number;
}

interface VolumeMsg {
	type: 'volume';
	volume: number;
	mute: boolean;
}

interface PauseMsg {
	type: 'pause';
}

interface DisposeMsg {
	type: 'dispose';
}

type InMsg = InitMsg | ScheduleMsg | VolumeMsg | PauseMsg | DisposeMsg;

class AudioSchedulerProcessor extends AudioWorkletProcessor {
	#buffer: AudioBuffer | null = null;
	#gain: GainNode | null = null;
	#ctx: AudioContext | null = null;
	#active: AudioBufferSourceNode[] = [];
	#muted = false;
	#volume = 1;

	#ensureContext(): { buffer: AudioBuffer; gain: GainNode; ctx: AudioContext } | null {
		if (this.#buffer && this.#gain && this.#ctx) {
			return { buffer: this.#buffer, gain: this.#gain, ctx: this.#ctx };
		}
		return null;
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

	#applyVolume(): void {
		if (!this.#gain) return;
		this.#gain.gain.value = this.#muted ? 0 : this.#volume;
	}

	receive(msg: InMsg): void {
		switch (msg.type) {
			case 'init': {
				// Build the AudioBuffer once. We rebuild on subsequent inits
				// (e.g. a new recording replaces the old one).
				this.#stopActive();
				const ctxCtor = (globalThis as { AudioContext?: typeof AudioContext }).AudioContext;
				if (!ctxCtor) return;
				const audioCtx = new ctxCtor({ sampleRate: msg.sampleRate, latencyHint: 'interactive' });
				const numChannels = msg.channels.length;
				const length = msg.channels[0]?.length ?? 0;
				const buffer = audioCtx.createBuffer(numChannels, length, msg.sampleRate);
				for (let c = 0; c < numChannels; c++) {
					// Copy the transferred channel into an `ArrayBuffer`-backed
					// `Float32Array`. The transferred buffer is contiguous in
					// practice, but TS 5.7+ types `Float32Array` invariantly
					// (`ArrayBufferLike` ≠ `ArrayBuffer`), and `copyToChannel`'s
					// signature predates the generic — the copy also avoids
					// `SharedArrayBuffer` aliasing edge cases.
					const src = msg.channels[c] ?? new Float32Array(0);
					const dst = new Float32Array(new ArrayBuffer(src.byteLength));
					dst.set(src);
					buffer.copyToChannel(dst, c);
				}
				const gain = audioCtx.createGain();
				gain.connect(audioCtx.destination);
				this.#ctx = audioCtx;
				this.#buffer = buffer;
				this.#gain = gain;
				this.#muted = msg.mute;
				this.#volume = msg.volume;
				this.#applyVolume();
				return;
			}
			case 'schedule': {
				const ctx = this.#ensureContext();
				if (!ctx) return;
				this.#stopActive();
				const ctxTime = currentTime;
				for (const c of msg.chunks) {
					// Clamp the slice to the buffer (tracks may be shorter than
					// the timeline).
					const bufDur = ctx.buffer.duration;
					if (c.bufferOffset >= bufDur) continue;
					const playDur = Math.min(c.duration, bufDur - c.bufferOffset);
					if (playDur <= 0) continue;
					const node = ctx.ctx.createBufferSource();
					node.buffer = ctx.buffer;
					node.playbackRate.value = c.rate;
					node.connect(ctx.gain);
					node.onended = (): void => {
						const i = this.#active.indexOf(node);
						if (i >= 0) this.#active.splice(i, 1);
					};
					node.start(ctxTime + c.whenDelay, c.bufferOffset, playDur);
					this.#active.push(node);
				}
				return;
			}
			case 'volume': {
				this.#volume = msg.volume;
				this.#muted = msg.mute;
				this.#applyVolume();
				return;
			}
			case 'pause': {
				this.#stopActive();
				return;
			}
			case 'dispose': {
				this.#stopActive();
				const ctx = this.#ctx;
				if (ctx && ctx.state !== 'closed') {
					void ctx.close().catch(() => {
						/* best-effort */
					});
				}
				this.#buffer = null;
				this.#gain = null;
				this.#ctx = null;
				return;
			}
		}
	}
}

registerProcessor('recast-audio-scheduler', AudioSchedulerProcessor);

// `currentFrame`/`sampleRate` are referenced by TS but the JS engine reads
// them at runtime; suppress the unused warning.
void sampleRate;
void currentFrame;