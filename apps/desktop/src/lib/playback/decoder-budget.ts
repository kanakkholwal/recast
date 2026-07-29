export interface DecoderLease {
	/** Pause (true) or resume (false) this consumer's decoding. */
	onPause: (paused: boolean) => void;
}

/** Should the secondary at `index` (0-based registration order) be paused, given
 *  preview activity and the concurrent-secondary cap? Pure — unit-tested. */
export function secondaryPaused(
	previewBusy: boolean,
	index: number,
	maxSecondary: number,
): boolean {
	if (previewBusy) return true;
	return index >= Math.max(1, maxSecondary);
}
/**
 * Shared hardware-decoder budget: caps concurrent WebCodecs decode
 * sessions and gives the preview priority, so secondary decoders (the filmstrip
 * today, browser export later) can't over-subscribe the GPU's limited decode
 * sessions — the root of the 4K/1080p open+scrub crash. The preview holds its
 * decoder for its whole lifetime; secondaries pause while the preview is busy
 * (playing OR scrubbing) and beyond the concurrency cap.
 *
 * This generalizes the old `setDecodePaused(isPlaying)` mutual-exclusion into one
 * shared policy that also covers scrubbing (preview seeks thrash the decoder
 * while `isPlaying` is false) and any future secondary consumer.
 */

export class DecoderBudget {
	#maxSecondary: number;
	#previewBusy = false;
	#secondaries: DecoderLease[] = [];

	constructor(maxSecondary = 1) {
		this.#maxSecondary = Math.max(1, maxSecondary);
	}

	/** Preview is decoding heavily (playing/scrubbing); secondaries must yield. */
	setPreviewBusy(busy: boolean): void {
		if (this.#previewBusy === busy) return;
		this.#previewBusy = busy;
		this.#reevaluate();
	}

	/** Register a pausable secondary decoder; returns an unregister fn. The lease
	 *  is pushed into its correct pause state immediately. */
	registerSecondary(lease: DecoderLease): () => void {
		this.#secondaries.push(lease);
		lease.onPause(
			secondaryPaused(this.#previewBusy, this.#secondaries.length - 1, this.#maxSecondary),
		);
		return () => {
			const i = this.#secondaries.indexOf(lease);
			if (i < 0) return;
			this.#secondaries.splice(i, 1);
			this.#reevaluate();
		};
	}

	#reevaluate(): void {
		this.#secondaries.forEach((lease, index) => {
			lease.onPause(secondaryPaused(this.#previewBusy, index, this.#maxSecondary));
		});
	}
}

/** Editor-wide singleton — preview + filmstrip (+ future export) share it. */
export const decoderBudget = new DecoderBudget(1);
