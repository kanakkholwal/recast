/**
 * Message protocol between the filmstrip provider (filmstrip-source.ts) and its
 * decode worker (filmstrip-worker.ts). Separate module so both sides share one
 * definition without importing the other's runtime code.
 */

/** Provider → worker. */
export type ToFilmstripWorker =
	/** The source video URL (range-streamed via MediaBunny's UrlSource — never
	 *  the whole file in memory), the device-pixel tile height to downscale each
	 *  thumbnail to, and the known duration so the worker skips a full container
	 *  walk to compute it. */
	| { type: "init"; url: string; tileHeightPx: number; durationSec?: number }
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
	| {
			type: "ready";
			width: number;
			height: number;
			durationSec: number;
			fps: number;
	  }
	/** A decoded, downscaled thumbnail as a compressed blob (cheap to clone; the
	 *  provider turns it into an object URL for an `<img>`). */
	| { type: "tile"; id: number; blob: Blob }
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
	| { type: "error"; message: string };
