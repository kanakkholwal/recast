/**
 * PlaybackClock: master timeline clock for the editor preview.
 *
 * A pure real-time integrator: while playing, `time` advances with wall-clock
 * at `rate`; paused, it holds. Nothing decodes here; the render loop samples
 * `time` and asks the video source for the matching frame; audio slaves to it.
 * Decoupling from `videoEl.currentTime` is the point: the decoder's seek
 * latency is what made playback freeze at cut boundaries.
 *
 * Knows nothing of cuts/trims; runs over a single gapless **output-time**
 * domain `[0, duration]` (recording with cuts removed; see timeline/cuts.ts).
 * The owner maps output→original via `outputToOriginal` where original media
 * time is still needed (frame lookup, cursor/zoom eval, audio `currentTime`).
 *
 * `now` is injectable so the integrator is unit-testable without real time.
 */

/** Monotonic millisecond time source. Defaults to `performance.now`. */
export type NowFn = () => number;

const defaultNow: NowFn = () =>
	typeof performance !== "undefined" ? performance.now() : Date.now();

export class PlaybackClock {
	#now: NowFn;
	// Anchor: at wall-clock `#anchorWallMs`, playback time was `#anchorTime`.
	// While playing, current time = anchorTime + (now - anchorWall)/1000 * rate.
	#anchorWallMs: number;
	#anchorTime: number;
	#playing = false;
	#rate = 1;
	#duration = 0;

	constructor(now: NowFn = defaultNow) {
		this.#now = now;
		this.#anchorWallMs = now();
		this.#anchorTime = 0;
	}

	/** Re-anchor so that, from this instant, `time` continues from `t`. */
	#reanchor(t: number): void {
		this.#anchorWallMs = this.#now();
		this.#anchorTime = this.#clamp(t);
	}

	#clamp(t: number): number {
		if (!(t > 0)) return 0; // also catches NaN
		if (t > this.#duration) return this.#duration;
		return t;
	}

	/** Current playback position in output-time seconds, clamped to [0, duration]. */
	get time(): number {
		if (!this.#playing) return this.#anchorTime;
		const elapsedSec = ((this.#now() - this.#anchorWallMs) / 1000) * this.#rate;
		return this.#clamp(this.#anchorTime + elapsedSec);
	}

	get playing(): boolean {
		return this.#playing;
	}

	get rate(): number {
		return this.#rate;
	}

	get duration(): number {
		return this.#duration;
	}

	/** True once playback has reached the end of the output domain. */
	get atEnd(): boolean {
		return this.#duration > 0 && this.time >= this.#duration - 1e-6;
	}

	/**
	 * Set the output duration. Re-anchors first so a duration change mid-playback
	 * doesn't retroactively move the current time; the new value re-clamps it.
	 */
	setDuration(duration: number): void {
		this.#reanchor(this.time);
		this.#duration = duration > 0 ? duration : 0;
		this.#anchorTime = this.#clamp(this.#anchorTime);
	}

	play(): void {
		if (this.#playing) return;
		this.#reanchor(this.time);
		this.#playing = true;
	}

	pause(): void {
		if (!this.#playing) return;
		this.#anchorTime = this.time; // freeze at the computed time
		this.#playing = false;
	}

	/** Jump to output time `t` (clamped). Preserves play/pause state. */
	seek(t: number): void {
		this.#reanchor(t);
	}

	/** Set playback rate (e.g. 0.5, 2). Re-anchors so the change is seamless. */
	setRate(rate: number): void {
		if (!(rate > 0)) return;
		this.#reanchor(this.time);
		this.#rate = rate;
	}
}
