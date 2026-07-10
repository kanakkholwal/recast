/**
 * Shared whole-file MP4 demux for the WebCodecs workers.
 *
 * Both the preview decoder (webcodecs-worker.ts) and the filmstrip decoder
 * (../timeline/filmstrip-worker.ts) need the same first step: mp4box parses the
 * file into encoded samples (decode order) plus the codec config. This owns that
 * one dance so neither worker reimplements it. The progressive (HTTP-range) path
 * stays in webcodecs-worker.ts; only whole-file ingestion is shared.
 */

import {
	createFile,
	DataStream,
	type ISOFile,
	MP4BoxBuffer,
	type Movie,
	type Sample,
	type Track,
} from "mp4box";
import type { ChunkMeta } from "./frame-index";

export interface DemuxResult {
	/** Encoded samples in decode order. */
	chunks: ChunkMeta[];
	/** Decoder config (codec + description), already checked as supported. */
	config: VideoDecoderConfig;
	/** Display width (px). */
	width: number;
	/** Display height (px). */
	height: number;
	/** Track duration (seconds). */
	durationSec: number;
	/** Average frame rate (samples / duration). */
	fps: number;
}

/**
 * Demux an in-memory MP4 into its sample table and decoder config. Throws if the
 * WebView lacks WebCodecs, the file has no decodable video track, or the codec
 * isn't supported; the caller should fall back to a non-WebCodecs path.
 */
export async function demuxWholeFile(ab: ArrayBuffer): Promise<DemuxResult> {
	if (typeof VideoDecoder === "undefined") {
		throw new Error("WebCodecs VideoDecoder unavailable");
	}
	const file = createFile();
	const collected: ChunkMeta[] = [];

	// Resolve with track + config so they reach the linear flow as non-null
	// locals; TS can't see assignments inside these mp4box callbacks.
	const ready = new Promise<{ track: Track; cfg: VideoDecoderConfig }>(
		(resolve, reject) => {
			let track: Track | null = null;
			let cfg: VideoDecoderConfig | null = null;
			file.onError = (e: string) => reject(new Error(`mp4box: ${e}`));
			file.onReady = (info: Movie) => {
				const vtrack =
					info.videoTracks?.[0] ??
					info.tracks.find((t) => t.type === "video") ??
					null;
				if (!vtrack) {
					reject(new Error("no video track in source"));
					return;
				}
				track = vtrack;
				cfg = {
					codec: vtrack.codec,
					description: extractDescription(file, vtrack.id),
					// Throughput over latency: a latency-optimised decoder serialises
					// and can tank to single-digit fps.
					hardwareAcceleration: "prefer-hardware",
					optimizeForLatency: false,
				};
				file.setExtractionOptions(vtrack.id, null, { nbSamples: Infinity });
				file.start();
			};
			file.onSamples = (_id: number, _user: unknown, samples: Sample[]) => {
				for (const s of samples) {
					const ts = s.timescale || 1;
					collected.push({
						ctsUs: Math.round((s.cts / ts) * 1e6),
						durUs: Math.round((s.duration / ts) * 1e6),
						key: s.is_sync,
						data: s.data ? s.data.slice() : new Uint8Array(0),
					});
				}
				if (track && cfg && collected.length >= track.nb_samples)
					resolve({ track, cfg });
			};
		},
	);

	file.appendBuffer(MP4BoxBuffer.fromArrayBuffer(ab, 0), true);
	file.flush();
	const { track, cfg } = await ready;

	if (collected.length === 0) throw new Error("demux produced no samples");
	const support = await VideoDecoder.isConfigSupported(cfg);
	if (!support.supported) throw new Error(`codec not supported: ${cfg.codec}`);

	const durationSec =
		track.movie_timescale > 0 ? track.movie_duration / track.movie_timescale : 0;
	const width = track.video?.width ?? track.track_width;
	const height = track.video?.height ?? track.track_height;
	return {
		chunks: collected,
		config: cfg,
		width,
		height,
		durationSec,
		fps: durationSec > 0 ? track.nb_samples / durationSec : 30,
	};
}

/**
 * Pull the codec config (avcC / hvcC / av1C / vpcC) out of the sample
 * description as the raw box payload WebCodecs expects (box contents minus the
 * 8-byte size+type header).
 */
export function extractDescription(file: ISOFile, trackId: number): Uint8Array {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const trak = file.getTrackById(trackId) as any;
	const entries = trak?.mdia?.minf?.stbl?.stsd?.entries ?? [];
	for (const entry of entries) {
		const box = entry.avcC ?? entry.hvcC ?? entry.av1C ?? entry.vpcC;
		if (box) {
			// Default endianness is BIG_ENDIAN (network order), as avcC requires.
			const stream = new DataStream(undefined, 0);
			box.write(stream);
			return new Uint8Array(stream.buffer, 8);
		}
	}
	throw new Error("no codec description (avcC/hvcC/...) in sample table");
}
