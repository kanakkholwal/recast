/**
 * Message protocol between the filmstrip provider (filmstrip-source.ts) and its
 * decode worker (filmstrip-worker.ts). Separate module so both sides share one
 * definition without importing the other's runtime code.
 */

import type { MediaRef } from "@recast/media";

/** Provider → worker. */
export type ToFilmstripWorker =
	/** The source ref (range-streamed or file-sliced — never the whole file in
	 *  memory), the device-pixel tile height to downscale each thumbnail to, and
	 *  the known duration so the worker skips a full container walk. */
	| { type: "init"; src: MediaRef; tileHeightPx: number; durationSec?: number }
	/** A batch of thumbnails to decode. Each `id` correlates the reply. The worker
	 *  groups them by GOP so one keyframe decode serves every tile in it. */
	| { type: "decode"; requests: Array<{ id: number; originalSec: number }> }
	/** Build a YouTube-style storyboard: one sprite sheet packing evenly-spaced
	 *  frames into a grid, so hover-scrub crops a cell instead of decoding a frame
	 *  per position. The worker picks the cell count/grid from the duration. */
	| { type: "storyboard" }
	| { type: "dispose" };

/** Worker → provider. */
export type FromFilmstripWorker =
	/** Init finished and the sink is live. Carries nothing: the provider already
	 *  knows the dimensions it asked for. */
	| { type: "ready" }
	/** A decoded, downscaled thumbnail as a compressed blob (cheap to clone; the
	 *  provider turns it into an object URL for an `<img>`). */
	| { type: "tile"; id: number; blob: Blob; width: number; height: number }
	/** The finished storyboard sprite: a single image of `cols`×`rows` cells, each
	 *  `cellW`×`cellH`, holding `count` frames evenly spaced across `durationSec`.
	 *  Cell `i` (col `i%cols`, row `i/cols`) samples time `((i+0.5)/count)·dur`. */
	| {
			type: "storyboard";
			blob: Blob;
			cols: number;
			rows: number;
			cellW: number;
			cellH: number;
			count: number;
			durationSec: number;
	  }
	/** `id` is set when a specific decode request failed, so the provider can
	 *  clear it from in-flight (allowing a retry) instead of leaking the entry.
	 *  Absent for init errors that aren't tied to one request. */
	| { type: "error"; message: string; id?: number }
	/** The storyboard build failed. Its own type because it carries no request
	 *  id: as a plain `error` the provider could not tell it apart, so the
	 *  one-shot latch stayed set and hover scrub never retried for the session. */
	| { type: "storyboard-error"; message: string }
	/** A queued request evicted unanswered because the scroll outran the decoder.
	 *  The provider must still clear it from in-flight or the tile wedges, but it
	 *  is not a failure and must not be logged as one. */
	| { type: "drop"; id: number };
