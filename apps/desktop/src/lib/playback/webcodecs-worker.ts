/// <reference lib="webworker" />
/**
 * WebCodecs decode worker: the off-main-thread half of the preview engine.
 *
 * The decode itself runs on the browser's internal threads; what we move here is
 * everything around it that would otherwise hit the WebView's main thread: the
 * mp4box demux, the sample-index build, the decoder `output()` callback, and the
 * decode-ahead scheduling. The main thread is left with the WebGL upload.
 *
 * The worker owns no decoded frames: each is transferred to the main thread the
 * instant it's decoded (transfer neuters the worker's handle, so no double-own
 * or leak). The worker owns the encoded samples + the decoder and schedules
 * feeding from the playhead time the main thread sends.
 */

import { createFile, type ISOFile, MP4BoxBuffer, type Movie } from "mp4box";
import {
	buildKeyframes,
	buildPresOrder,
	type ChunkMeta,
	keyframeAtOrBefore,
	needsReset,
	sampleAtOrBefore,
} from "./frame-index";
import { frameBudget } from "./frame-budget";
import { GopByteBudget } from "./gop-byte-budget";
import { demuxWholeFile, extractDescription } from "./mp4-demux";
import {
	buildSampleTable,
	gopByteRange,
	type RawSampleTables,
	type SampleEntry,
} from "./mp4-sample-table";
import type { FromWorker, ToWorker } from "./webcodecs-protocol";

/** Samples (decode order) kept decoded ahead of the playhead. Small so we don't
 * hold many output surfaces (stalls HW decode). Resolution-adaptive: set from
 * `frameBudget` at init so 4K/5K decodes fewer ahead. Defaults to the 1080p value. */
let decodeAhead = 6;
/** Cap on the decoder's in-flight queue so we don't overfeed during a burst. */
const QUEUE_MAX = 4;
/**
 * A forward jump in requested time bigger than this (µs) is treated as a seek or
 * cut: the only case where we reset the decoder to a downstream keyframe and
 * skip ahead. A smaller forward step is normal playback or a lagging decoder;
 * there we keep streaming contiguously so no on-screen content is skipped. (A
 * cut shorter than this streams through its removed frames, which the
 * compositor's segment floor hides anyway.)
 */
const SEEK_JUMP_US = 300_000;

/** Bytes mp4box is fed per range request while locating the `moov` index. Big
 * enough to swallow ftyp + the mdat header in one shot so a moov-at-end file
 * resolves in two fetches (head, then the moov tail). */
const MOOV_FETCH_CHUNK = 256 * 1024;
/**
 * Tier-1 budget (bytes) for the progressive path's cache of *encoded* GOP bytes.
 * Generous because encoded bytes are cheap CPU RAM with no decoder-surface
 * pressure (that's the separate, count-bounded decoded-frame cache); this is what
 * makes scrubbing back over a cut free.
 */
const TIER1_BUDGET_BYTES = 256 * 1024 * 1024;

const ctx = self as unknown as DedicatedWorkerGlobalScope;

// Dev-only diagnostics: decoder output rate, queue depth, reset count, to tell
// whether the decoder is slow vs. frames arriving late on main.
const DIAG = import.meta.env.DEV;
let diagDecoded = 0;
let diagResets = 0;
let diagLastLogMs = 0;

let chunks: ChunkMeta[] = [];
let keyframes: number[] = [];
let presOrder: number[] = [];
let decoder: VideoDecoder | null = null;
let config: VideoDecoderConfig | null = null;
let anchorKey = -1;
let feedCursor = 0;
let currentUs = 0;
/** Requested time (µs) at the previous schedule(), to tell a seek/cut jump from
 * smooth playback (incl. a lagging decoder). -1 until the first request. */
let lastScheduleUs = -1;
let disposed = false;

// Progressive (HTTP-range) ingestion state. Unused in whole-file mode.
/** True once `init-progressive` set up a lazy, range-fetched sample table. */
let progressive = false;
/** Asset URL to range-fetch GOP media bytes from. */
let sourceUrl = "";
/** Per-sample byte map (offset/size), parallel to `chunks`, for range-fetching
 * media bytes on demand. Empty in whole-file mode. */
let sampleTable: SampleEntry[] = [];
/** Tier-1 LRU over encoded GOP bytes (keyed by the GOP's keyframe decode-index). */
let byteBudget: GopByteBudget | null = null;
/** In-flight GOP fetches, keyed by keyframe decode-index, for cancellation. */
const inflight = new Map<number, AbortController>();
/** GOPs whose media bytes are currently resident in `chunks[].data`. */
const gopLoaded = new Set<number>();
/** Bumped on every decoder reset so bytes from a superseded position are dropped
 * when their fetch finally resolves (the racing-scrub guard). */
let currentGen = 0;
/** Last schedule() target, so a completed GOP fetch can resume feeding. */
let lastTarget = 0;

// Scout decoder (cross-cut decode-ahead): pre-decodes the first displayable
// frame(s) of an upcoming post-cut GOP into the main cache so crossing the cut
// doesn't freeze while the primary re-decodes from a keyframe. Self-disables on
// any error, falling back to the primary-only "hold last frame" behaviour.
let scoutDecoder: VideoDecoder | null = null;
/** Keyframe decode-index the scout is currently warming (dedup). -1 = idle. */
let scoutAnchorKf = -1;
/** Only scout frames at/after this presentation time (µs) are forwarded: the
 * displayable post-cut frame onward; earlier GOP-internal frames are decode
 * dependencies we don't show. */
let scoutTargetTs = Number.POSITIVE_INFINITY;
/** Cleared if the scout ever errors, so a flaky decoder can't thrash playback. */
let scoutEnabled = true;

function post(msg: FromWorker, transfer: Transferable[] = []): void {
	ctx.postMessage(msg, transfer);
}

async function init(ab: ArrayBuffer): Promise<void> {
	try {
		const res = await demuxWholeFile(ab);
		chunks = res.chunks;
		keyframes = buildKeyframes(chunks);
		presOrder = buildPresOrder(chunks);
		config = res.config;
		decoder = makeDecoder();
		decoder.configure(config);
		decodeAhead = frameBudget(res.width, res.height).decodeAhead;
		post({
			type: "ready",
			width: res.width,
			height: res.height,
			durationSec: res.durationSec,
			fps: res.fps,
		});
	} catch (err) {
		post({ type: "error", message: err instanceof Error ? err.message : String(err) });
	}
}

/** The shared VideoDecoder (same wiring for both ingestion paths): transfer each
 * decoded frame to the main thread, log throughput in DEV. */
function makeDecoder(): VideoDecoder {
	return new VideoDecoder({
		output: (frame) => {
			if (disposed) {
				frame.close();
				return;
			}
			if (DIAG) {
				diagDecoded++;
				const nowMs = performance.now();
				if (nowMs - diagLastLogMs > 1000) {
					console.log(
						`[wc-worker] decoded ${diagDecoded}/s · queue=${decoder?.decodeQueueSize} · resets=${diagResets} · fed=${feedCursor}/${chunks.length} · mode=${progressive ? "progressive" : "whole"}${progressive ? ` · gopBytes=${((byteBudget?.totalBytes ?? 0) / 1e6).toFixed(0)}MB · inflight=${inflight.size}` : ""}`,
					);
					diagDecoded = 0;
					diagResets = 0;
					diagLastLogMs = nowMs;
				}
			}
			// Transfer to main; do NOT close after; transfer detaches our handle.
			post({ type: "frame", frame }, [frame]);
		},
		error: (e) => post({ type: "error", message: String(e) }),
	});
}

/** The scout decoder: forwards only frames at/after `scoutTargetTs` (the
 * displayable post-cut frame onward) tagged `fromScout`, and disables scouting on
 * any error rather than disturbing playback. */
function makeScoutDecoder(): VideoDecoder {
	return new VideoDecoder({
		output: (frame) => {
			// Drop the GOP-internal frames before the display target; they're only
			// decode dependencies. Forward the rest into the main cache's holdout.
			if (disposed || frame.timestamp < scoutTargetTs) {
				frame.close();
				return;
			}
			post({ type: "frame", frame, fromScout: true }, [frame]);
		},
		error: (e) => {
			scoutEnabled = false;
			scoutAnchorKf = -1;
			try {
				if (scoutDecoder && scoutDecoder.state !== "closed") scoutDecoder.close();
			} catch {
				/* already closing */
			}
			scoutDecoder = null;
			if (DIAG) console.warn("[wc-worker] scout disabled after error:", e);
		},
	});
}

/**
 * Fetch a byte range from the asset URL. Throws if the server ignored the Range
 * header (status 200 with the whole body); that means progressive ingestion is
 * impossible on this platform and the caller should fall back to `<video>`.
 */
async function fetchRange(
	url: string,
	startByte: number,
	endByte: number,
	signal?: AbortSignal,
): Promise<ArrayBuffer> {
	const res = await fetch(url, {
		headers: { Range: `bytes=${startByte}-${endByte}` },
		signal,
	});
	if (res.status === 200) {
		throw new Error("asset protocol ignored Range header (got 200, not 206)");
	}
	if (!res.ok && res.status !== 206) {
		throw new Error(`range fetch failed: HTTP ${res.status}`);
	}
	return res.arrayBuffer();
}

/** Read the raw `stbl` sample-table arrays off the parsed mp4box box tree. */
function extractSampleTables(file: ISOFile, trackId: number): RawSampleTables {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const trak = file.getTrackById(trackId) as any;
	const stbl = trak?.mdia?.minf?.stbl;
	const mdhd = trak?.mdia?.mdhd;
	if (!stbl || !mdhd) throw new Error("missing stbl/mdhd in track");
	const stsz = stbl.stsz;
	const stco = stbl.stco ?? stbl.co64;
	const stsc = stbl.stsc;
	const stts = stbl.stts;
	const ctts = stbl.ctts;
	const stss = stbl.stss;
	if (!stsz || !stco || !stsc || !stts) {
		throw new Error("incomplete sample tables (stsz/stco/stsc/stts)");
	}
	return {
		sampleSizes: stsz.sample_sizes ?? [],
		sampleSizeConstant: stsz.sample_size ?? 0,
		sampleCount: stsz.sample_count ?? (stsz.sample_sizes?.length ?? 0),
		chunkOffsets: stco.chunk_offsets ?? [],
		stscFirstChunk: stsc.first_chunk ?? [],
		stscSamplesPerChunk: stsc.samples_per_chunk ?? [],
		sttsCounts: stts.sample_counts ?? [],
		sttsDeltas: stts.sample_deltas ?? [],
		cttsCounts: ctts?.sample_counts,
		cttsOffsets: ctts?.sample_offsets,
		stssSampleNumbers: stss?.sample_numbers,
		timescale: mdhd.timescale,
	};
}

/**
 * Progressive ingestion: fetch only the `moov` index up front via Range requests
 * (driven by mp4box's next-byte position, which skips a front mdat to reach a
 * tail moov), build the sample byte-map, then range-fetch media per GOP on demand.
 */
async function initProgressive(url: string, sizeBytes: number): Promise<void> {
	if (typeof VideoDecoder === "undefined") {
		post({ type: "error", message: "WebCodecs VideoDecoder unavailable" });
		return;
	}
	try {
		const file = createFile();
		let info: Movie | null = null;
		let parseError: string | null = null;
		file.onError = (e: string) => {
			parseError = e;
		};
		file.onReady = (movie: Movie) => {
			info = movie;
		};

		// Feed ranges until mp4box parses the moov. appendBuffer returns the next
		// position it needs; for a tail moov that jumps past the mdat payload so
		// we don't download media to find the index.
		let pos = 0;
		while (!info && pos < sizeBytes) {
			const end = Math.min(pos + MOOV_FETCH_CHUNK, sizeBytes) - 1;
			const ab = await fetchRange(url, pos, end);
			const next = file.appendBuffer(MP4BoxBuffer.fromArrayBuffer(ab, pos), false);
			if (parseError) throw new Error(`mp4box: ${parseError}`);
			if (info) break;
			pos = next > end + 1 ? next : end + 1;
		}
		const movie = info as Movie | null;
		if (!movie) throw new Error("moov index not found within file");

		const vtrack =
			movie.videoTracks?.[0] ?? movie.tracks.find((t) => t.type === "video") ?? null;
		if (!vtrack) throw new Error("no video track in source");

		const cfg: VideoDecoderConfig = {
			codec: vtrack.codec,
			description: extractDescription(file, vtrack.id),
			hardwareAcceleration: "prefer-hardware",
			optimizeForLatency: false,
		};
		const support = await VideoDecoder.isConfigSupported(cfg);
		if (!support.supported) throw new Error(`codec not supported: ${cfg.codec}`);

		const table = buildSampleTable(extractSampleTables(file, vtrack.id));
		if (table.length === 0) throw new Error("could not build sample index");

		sampleTable = table;
		chunks = table.map((s) => ({
			ctsUs: s.ctsUs,
			durUs: s.durUs,
			key: s.key,
			data: null,
		}));
		keyframes = buildKeyframes(chunks);
		presOrder = buildPresOrder(chunks);
		config = cfg;
		progressive = true;
		sourceUrl = url;
		byteBudget = new GopByteBudget(TIER1_BUDGET_BYTES);
		decoder = makeDecoder();
		decoder.configure(config);

		const durationSec =
			vtrack.movie_timescale > 0 ? vtrack.movie_duration / vtrack.movie_timescale : 0;
		const vw = vtrack.video?.width ?? vtrack.track_width;
		const vh = vtrack.video?.height ?? vtrack.track_height;
		decodeAhead = frameBudget(vw, vh).decodeAhead;
		post({
			type: "ready",
			width: vw,
			height: vh,
			durationSec,
			fps: durationSec > 0 ? vtrack.nb_samples / durationSec : 30,
		});
	} catch (err) {
		post({ type: "error", message: err instanceof Error ? err.message : String(err) });
	}
}

/** The keyframe decode-index that ends the GOP starting at `kfIndex` (exclusive),
 * or chunks.length at end of stream. */
function gopEnd(kfIndex: number): number {
	const ki = keyframes.indexOf(kfIndex);
	return ki >= 0 && ki + 1 < keyframes.length ? keyframes[ki + 1] : chunks.length;
}

/** Release a resident GOP's encoded bytes (Tier-1 eviction). Never touches the
 * GOP currently feeding (the caller protects it via the budget). */
function freeGop(kfIndex: number): void {
	const end = gopEnd(kfIndex);
	for (let i = kfIndex; i < end; i++) chunks[i].data = null;
	gopLoaded.delete(kfIndex);
}

/** Range-fetch one GOP's media bytes and slice them into its samples, then
 * resume feeding. Guarded by a generation token so bytes for a superseded
 * position (after a reset/seek) are discarded on arrival. */
async function loadGop(kfIndex: number): Promise<void> {
	const { startByte, endByte } = gopByteRange(sampleTable, keyframes, kfIndex);
	if (endByte < startByte) return;
	const gen = currentGen;
	const controller = new AbortController();
	inflight.set(kfIndex, controller);
	try {
		const buf = await fetchRange(sourceUrl, startByte, endByte, controller.signal);
		if (disposed || gen !== currentGen) return; // superseded, drop it
		const view = new Uint8Array(buf);
		const end = gopEnd(kfIndex);
		for (let i = kfIndex; i < end; i++) {
			const s = sampleTable[i];
			if (!s) continue;
			const rel = s.offset - startByte;
			chunks[i].data = view.subarray(rel, rel + s.size);
		}
		gopLoaded.add(kfIndex);
		if (byteBudget) {
			for (const ev of byteBudget.touch(kfIndex, endByte - startByte + 1, anchorKey)) {
				freeGop(ev);
			}
		}
		schedule(lastTarget); // bytes are here, continue feeding
	} catch (err) {
		if (!disposed && (err as { name?: string })?.name !== "AbortError" && DIAG) {
			console.warn("[wc-worker] GOP load failed", kfIndex, err);
		}
	} finally {
		// Only clear if a newer load for this GOP hasn't replaced us.
		if (inflight.get(kfIndex) === controller) inflight.delete(kfIndex);
	}
}

/** Ensure the GOP at keyframe `kfIndex` has its media bytes, fetching if needed. */
function ensureGopLoaded(kfIndex: number): void {
	if (gopLoaded.has(kfIndex) || inflight.has(kfIndex)) return;
	void loadGop(kfIndex);
}

/** Cancel all in-flight GOP fetches and bump the generation (on a reset/seek so
 * their late bytes are ignored). Resident GOP bytes survive; that's the cache. */
function cancelInflight(): void {
	currentGen++;
	for (const c of inflight.values()) c.abort();
	inflight.clear();
}

/** Ensure the decoder is feeding the GOP that covers decode-index `target`. */
function schedule(target: number): void {
	if (!decoder || !config) return;
	lastTarget = target;
	const kf = keyframeAtOrBefore(keyframes, target);
	// A forward GOP gap is a reset-worthy seek/cut only if the time JUMPED; a
	// smooth advance or a lagging decoder streams on (see needsReset).
	const forwardIsJump =
		lastScheduleUs < 0 || Math.abs(currentUs - lastScheduleUs) > SEEK_JUMP_US;
	lastScheduleUs = currentUs;
	if (needsReset(anchorKey, feedCursor, kf, forwardIsJump)) {
		// Cancel in-flight GOP fetches so their late bytes are dropped; resident
		// GOP bytes stay cached (that's what makes scrubbing back over a cut free).
		if (progressive) cancelInflight();
		decoder.reset();
		decoder.configure(config); // reset() drops the config
		anchorKey = kf;
		feedCursor = kf;
		// The primary moved; re-arm scouting so the next approaching cut warms
		// again (and a replay of the same cut isn't blocked by a stale dedup).
		scoutAnchorKf = -1;
		diagResets++;
	}
	const end = Math.min(target + decodeAhead, chunks.length - 1);
	while (feedCursor <= end && decoder.decodeQueueSize < QUEUE_MAX) {
		const c = chunks[feedCursor];
		// Progressive: if this GOP's media bytes aren't resident yet, kick off a
		// range fetch and stop; loadGop() resumes feeding when the bytes land.
		if (progressive && c.data === null) {
			ensureGopLoaded(keyframeAtOrBefore(keyframes, feedCursor));
			break;
		}
		decoder.decode(
			new EncodedVideoChunk({
				type: c.key ? "key" : "delta",
				timestamp: c.ctsUs,
				duration: c.durUs,
				data: c.data as Uint8Array,
			}),
		);
		feedCursor++;
	}
}

function request(originalSec: number): void {
	if (disposed || chunks.length === 0) return;
	currentUs = Math.max(0, Math.round(originalSec * 1e6));
	schedule(sampleAtOrBefore(chunks, presOrder, currentUs));
}

function prefetch(originalSec: number): void {
	if (disposed || !scoutEnabled || chunks.length === 0 || !config) return;
	const tUs = Math.max(0, Math.round(originalSec * 1e6));
	const target = sampleAtOrBefore(chunks, presOrder, tUs);
	const kf = keyframeAtOrBefore(keyframes, target);
	// Nothing to warm if the primary already covers this GOP, or we're already
	// warming it.
	if (kf === anchorKey || kf === scoutAnchorKf) return;
	// Progressive: the GOP's media bytes must be resident first. Kick the loader
	// and bail; the next prefetch tick retries once the bytes land.
	if (progressive) {
		const end = gopEnd(kf);
		for (let i = kf; i < end; i++) {
			if (chunks[i].data === null) {
				ensureGopLoaded(kf);
				return;
			}
		}
	}
	scoutAnchorKf = kf;
	// Only emit the displayable post-cut frame (sampleAtOrBefore the cut point)
	// and beyond; earlier frames in the GOP are decode dependencies.
	scoutTargetTs = chunks[target].ctsUs;
	if (!scoutDecoder || scoutDecoder.state === "closed") scoutDecoder = makeScoutDecoder();
	try {
		scoutDecoder.reset();
		scoutDecoder.configure(config);
	} catch (e) {
		scoutEnabled = false;
		scoutAnchorKf = -1;
		if (DIAG) console.warn("[wc-worker] scout configure failed:", e);
		return;
	}
	// Feed the GOP from its keyframe through a couple frames past the target, so a
	// few post-cut frames are cached to bridge the jump until the primary catches
	// up (it re-decodes the same GOP itself on the actual cut crossing).
	const feedEnd = Math.min(target + 2, chunks.length - 1);
	for (let i = kf; i <= feedEnd; i++) {
		const c = chunks[i];
		if (c.data === null) break; // progressive byte gap, retry next tick
		scoutDecoder.decode(
			new EncodedVideoChunk({
				type: c.key ? "key" : "delta",
				timestamp: c.ctsUs,
				duration: c.durUs,
				data: c.data as Uint8Array,
			}),
		);
	}
}

function dispose(): void {
	if (disposed) return;
	disposed = true;
	for (const c of inflight.values()) c.abort();
	inflight.clear();
	gopLoaded.clear();
	byteBudget?.clear();
	try {
		if (decoder && decoder.state !== "closed") decoder.close();
		if (scoutDecoder && scoutDecoder.state !== "closed") scoutDecoder.close();
	} catch {
		/* already closing */
	}
	decoder = null;
	scoutDecoder = null;
	chunks = [];
	sampleTable = [];
	ctx.close();
}

ctx.onmessage = (e: MessageEvent<ToWorker>) => {
	const msg = e.data;
	switch (msg.type) {
		case "init":
			void init(msg.buffer);
			break;
		case "init-progressive":
			void initProgressive(msg.url, msg.sizeBytes);
			break;
		case "request":
			request(msg.originalSec);
			break;
		case "prefetch":
			prefetch(msg.originalSec);
			break;
		case "dispose":
			dispose();
			break;
	}
};
