/**
 * AudioWorklet-backed scheduler: sample-accurate audio playback that survives
 * main-thread jank. The main thread decodes audio files (one fetch + one
 * `decodeAudioData`), posts the raw channel buffers + scheduling commands to
 * the worklet, and never touches the audio clock after that.
 *
 * Lifecycle mirrors `audio-engine.ts` so `VideoPreview.svelte` can swap
 * implementations without touching call sites. The fallback
 * `FallbackAudioScheduler` (in this same file) preserves the JS-thread
 * path for environments where AudioWorklet isn't available.
 *
 * Worker file location: the host owns the worker URL via `AudioSchedulerConfig`.
 * The desktop app points this at `apps/desktop/src/lib/playback/audio-worklet-processor.ts`
 * (bundled by Vite as a module worker).
 */

import type { Region } from './schedule';
import { planAudioSchedule } from './schedule';

export interface AudioSchedulerConfig {
	/** URL of the AudioWorklet processor module. Vite-friendly string or URL. */
	workletUrl: string | URL;
	/** Optional volume (0..100) and mute at init time. */
	volume?: number;
	muted?: boolean;
}

export interface AudioScheduler {
	/** True once the audio buffers + AudioContext are ready. */
	readonly ready: boolean;

	/** Load and decode the audio sources. Throws on no decodable tracks. */
	load(urls: ReadonlyArray<string | null | undefined>): Promise<void>;

	/** Start (or restart) playback from OUTPUT time `fromOutputTime`. */
	play(regions: ReadonlyArray<Region>, fromOutputTime: number): Promise<void>;

	/** Stop all sound; keep buffers for the next play. */
	pause(): void;

	/** Re-plan the schedule (cuts changed, user scrubbed). No-op while paused. */
	reschedule(regions: ReadonlyArray<Region>, fromOutputTime: number): void;

	/** Apply volume (0..100) and mute to the gain node. */
	setVolume(volume0to100: number, muted: boolean): void;

	/** Tear down the worklet + AudioContext. */
	dispose(): Promise<void>;

	/** True when this scheduler is using AudioWorklet (vs. the JS-thread fallback). */
	readonly backend: 'worklet' | 'fallback';
}

class WorkletAudioScheduler implements AudioScheduler {
	#ctx: AudioContext | null = null;
	#node: AudioWorkletNode | null = null;
	#ready = false;
	#buffer: AudioBuffer | null = null;
	#volume = 1;
	#muted = false;

	constructor(private readonly config: AudioSchedulerConfig) {
		this.#volume = (config.volume ?? 100) / 100;
		this.#muted = config.muted ?? false;
	}

	get ready(): boolean {
		return this.#ready;
	}

	get backend(): 'worklet' | 'fallback' {
		return 'worklet';
	}

	async load(urls: ReadonlyArray<string | null | undefined>): Promise<void> {
		const Ctx: typeof AudioContext | undefined =
			typeof AudioContext !== 'undefined'
				? AudioContext
				: ((globalThis as { webkitAudioContext?: typeof AudioContext })
						.webkitAudioContext as typeof AudioContext | undefined);
		if (!Ctx) throw new Error('Web Audio API unavailable');
		if (typeof Ctx.prototype.audioWorklet === 'undefined') {
			throw new Error('AudioWorklet not supported in this WebView');
		}

		const ctx = new Ctx({ latencyHint: 'interactive' });
		await ctx.audioWorklet.addModule(this.config.workletUrl as string);
		const node = new AudioWorkletNode(ctx, 'recast-audio-scheduler');
		node.connect(ctx.destination);
		this.#ctx = ctx;
		this.#node = node;

		const buffers: AudioBuffer[] = [];
		for (const url of urls) {
			if (!url) continue;
			try {
				const res = await fetch(url);
				if (!res.ok) continue;
				const data = await res.arrayBuffer();
				const buf = await ctx.decodeAudioData(data);
				buffers.push(buf);
			} catch {
				/* skip */
			}
		}
		if (buffers.length === 0) {
			try {
				await ctx.close();
			} catch {
				/* ignore */
			}
			throw new Error('no decodable audio tracks');
		}

		// Merge all decoded tracks into a single buffer (sum their channels).
		// Multi-track mixing keeps the legacy behavior — the editor's per-track
		// gain is mixed into a single gain node downstream.
		const numChannels = Math.max(...buffers.map((b) => b.numberOfChannels));
		const sampleRate = buffers[0]!.sampleRate;
		const length = Math.max(...buffers.map((b) => b.length));
		const merged = ctx.createBuffer(numChannels, length, sampleRate);
		for (const buf of buffers) {
			for (let c = 0; c < buf.numberOfChannels; c++) {
				const src = buf.getChannelData(c);
				const dst = merged.getChannelData(c);
				for (let i = 0; i < src.length; i++) dst[i] = (dst[i] ?? 0) + (src[i] ?? 0);
			}
		}
		this.#buffer = merged;

		// Transfer the channel data to the worklet. TypeScript 5.7+ types
		// `Float32Array` as `Float32Array<ArrayBufferLike>`, and the
		// `postMessage` transfer list requires `Float32Array<ArrayBuffer>`.
		// Copy each channel into a fresh `ArrayBuffer`-backed Float32Array so
		// the transfer typechecks and the transfer list maps cleanly.
		const channels: Float32Array<ArrayBuffer>[] = [];
		const transferList: ArrayBuffer[] = [];
		for (let c = 0; c < numChannels; c++) {
			const src = merged.getChannelData(c);
			const dst = new Float32Array(new ArrayBuffer(src.byteLength));
			dst.set(src);
			channels.push(dst);
			transferList.push(dst.buffer);
		}

		node.port.postMessage(
			{
				type: 'init',
				sampleRate,
				channels,
				mute: this.#muted,
				volume: this.#volume,
			},
			transferList,
		);

		this.#ready = true;
	}

	async play(regions: ReadonlyArray<Region>, fromOutputTime: number): Promise<void> {
		if (!this.#ready || !this.#node || !this.#ctx) return;
		if (this.#ctx.state === 'suspended') {
			try {
				await this.#ctx.resume();
			} catch {
				/* may reject before user activation; schedule anyway */
			}
		}
		const chunks = planAudioSchedule(regions, fromOutputTime);
		this.#node.port.postMessage({ type: 'schedule', chunks });
	}

	pause(): void {
		this.#node?.port.postMessage({ type: 'pause' });
	}

	reschedule(regions: ReadonlyArray<Region>, fromOutputTime: number): void {
		if (!this.#ready) return;
		const chunks = planAudioSchedule(regions, fromOutputTime);
		this.#node?.port.postMessage({ type: 'schedule', chunks });
	}

	setVolume(volume0to100: number, muted: boolean): void {
		this.#volume = Math.max(0, Math.min(1, volume0to100 / 100));
		this.#muted = muted;
		this.#node?.port.postMessage({ type: 'volume', volume: this.#volume, mute: this.#muted });
	}

	async dispose(): Promise<void> {
		try {
			this.#node?.port.postMessage({ type: 'dispose' });
		} catch {
			/* ignore */
		}
		this.#node?.disconnect();
		this.#node = null;
		if (this.#ctx && this.#ctx.state !== 'closed') {
			try {
				await this.#ctx.close();
			} catch {
				/* ignore */
			}
		}
		this.#ctx = null;
		this.#buffer = null;
		this.#ready = false;
	}
}

/**
 * JS-thread fallback. Mirrors the legacy `audio-engine.ts` API without
 * AudioWorklet. Used when:
 *   - The webview reports no `audioWorklet` on AudioContext.
 *   - `AudioWorklet.addModule()` rejects (browser policy, sandboxed iframe,
 *     missing cross-origin isolation for some features).
 *
 * Schedule math is the same as the worklet path (`planAudioSchedule`); only
 * the scheduling primitive differs.
 */
class FallbackAudioScheduler implements AudioScheduler {
	#ctx: AudioContext | null = null;
	#tracks: Array<{ buffer: AudioBuffer; gain: GainNode }> = [];
	#active: AudioBufferSourceNode[] = [];
	#ready = false;

	get ready(): boolean {
		return this.#ready;
	}

	get backend(): 'worklet' | 'fallback' {
		return 'fallback';
	}

	async load(urls: ReadonlyArray<string | null | undefined>): Promise<void> {
		const Ctx: typeof AudioContext | undefined =
			typeof AudioContext !== 'undefined'
				? AudioContext
				: ((globalThis as { webkitAudioContext?: typeof AudioContext })
						.webkitAudioContext as typeof AudioContext | undefined);
		if (!Ctx) throw new Error('Web Audio API unavailable');
		const ctx = new Ctx();
		const tracks: Array<{ buffer: AudioBuffer; gain: GainNode }> = [];
		for (const url of urls) {
			if (!url) continue;
			try {
				const res = await fetch(url);
				if (!res.ok) continue;
				const data = await res.arrayBuffer();
				const buffer = await ctx.decodeAudioData(data);
				const gain = ctx.createGain();
				gain.connect(ctx.destination);
				tracks.push({ buffer, gain });
			} catch {
				/* skip */
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
		this.#ctx = ctx;
		this.#tracks = tracks;
		this.#ready = true;
	}

	async play(regions: ReadonlyArray<Region>, fromOutputTime: number): Promise<void> {
		if (!this.#ready || !this.#ctx) return;
		if (this.#ctx.state === 'suspended') {
			try {
				await this.#ctx.resume();
			} catch {
				/* best-effort */
			}
		}
		this.#schedule(regions, fromOutputTime);
	}

	pause(): void {
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

	reschedule(regions: ReadonlyArray<Region>, fromOutputTime: number): void {
		this.#schedule(regions, fromOutputTime);
	}

	setVolume(volume0to100: number, muted: boolean): void {
		const v = muted ? 0 : Math.max(0, Math.min(1, volume0to100 / 100));
		for (const t of this.#tracks) t.gain.gain.value = v;
	}

	async dispose(): Promise<void> {
		this.pause();
		this.#tracks = [];
		if (this.#ctx && this.#ctx.state !== 'closed') {
			try {
				await this.#ctx.close();
			} catch {
				/* ignore */
			}
		}
		this.#ctx = null;
		this.#ready = false;
	}

	#schedule(regions: ReadonlyArray<Region>, from: number): void {
		for (const node of this.#active) {
			try {
				node.onended = null;
				node.stop();
			} catch {
				/* already stopped */
			}
		}
		this.#active = [];
		if (!this.#ctx) return;
		const now = this.#ctx.currentTime;
		const chunks = planAudioSchedule(regions, from);
		for (const t of this.#tracks) {
			const bufDur = t.buffer.duration;
			for (const c of chunks) {
				if (c.bufferOffset >= bufDur) continue;
				const playDur = Math.min(c.duration, bufDur - c.bufferOffset);
				if (playDur <= 0) continue;
				const node = this.#ctx.createBufferSource();
				node.buffer = t.buffer;
				node.playbackRate.value = c.rate;
				node.connect(t.gain);
				node.onended = (): void => {
					const i = this.#active.indexOf(node);
					if (i >= 0) this.#active.splice(i, 1);
				};
				node.start(now + c.whenDelay, c.bufferOffset, playDur);
				this.#active.push(node);
			}
		}
	}
}

/**
 * Factory. Returns a worklet-backed scheduler when `config.workletUrl` is
 * provided AND the runtime supports AudioWorklet; otherwise falls back to the
 * JS-thread implementation. The returned object's `backend` reports which
 * path it took — telemetry can use this to decide when to drop the fallback.
 */
export async function createAudioScheduler(
	config?: AudioSchedulerConfig,
): Promise<AudioScheduler> {
	if (
		config?.workletUrl &&
		typeof AudioContext !== 'undefined' &&
		AudioContext.prototype.audioWorklet !== undefined
	) {
		const scheduler = new WorkletAudioScheduler(config);
		// Caller invokes `load()` next; we don't probe `addModule` here so the
		// fallback decision is made on actual usage.
		return scheduler;
	}
	return new FallbackAudioScheduler();
}

export type { Region };