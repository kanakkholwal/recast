/**
 * Runtime WebCodecs capability detection for the browser conversion tools.
 *
 * Do NOT gate on the user agent or version numbers. WebCodecs support varies by
 * browser AND by codec, so the only reliable check is to ask the browser at
 * runtime: is the API present, and does it support THIS codec at THIS size? That
 * is what `isConfigSupported` is for. This module wraps those async probes and
 * maps a tool's requirements to a single supported / unsupported answer the UI
 * can render a banner from.
 *
 * Capability tiers (see `worker-protocol.ts` for which op is which):
 *   - container: rewrap/copy streams, no codec needed -> works everywhere.
 *   - decode:    needs Video/AudioDecoder -> broad (Chrome, Edge, Safari, recent FF).
 *   - encode:    needs VideoEncoder -> Chromium-first today.
 */

export type CapabilityTier = "container" | "decode" | "encode";

/** A codec + (for video) a representative size to probe. */
export interface VideoCodecCheck {
	codec: string;
	width?: number;
	height?: number;
	bitrate?: number;
}
export interface AudioCodecCheck {
	codec: string;
	sampleRate?: number;
	numberOfChannels?: number;
	bitrate?: number;
}

/** What a given tool needs to run. Only the fields relevant to its tier matter. */
export interface ToolRequirements {
	tier: CapabilityTier;
	videoDecode?: VideoCodecCheck;
	videoEncode?: VideoCodecCheck;
	audioDecode?: AudioCodecCheck;
	audioEncode?: AudioCodecCheck;
}

export type CapabilityStatus =
	| { supported: true }
	| { supported: false; reason: string };

//  API presence (cheap, synchronous) 

const present = (name: string): boolean =>
	typeof (globalThis as Record<string, unknown>)[name] !== "undefined";

export const hasVideoDecoder = (): boolean => present("VideoDecoder");
export const hasVideoEncoder = (): boolean => present("VideoEncoder");
export const hasAudioDecoder = (): boolean => present("AudioDecoder");
export const hasAudioEncoder = (): boolean => present("AudioEncoder");
export const hasImageDecoder = (): boolean => present("ImageDecoder");
/** Workers + transferable VideoFrames — the floor for any decode/encode tool. */
export const hasWorkers = (): boolean => present("Worker");

//  Per-codec probes (async, authoritative) 

export async function probeVideoDecode(c: VideoCodecCheck): Promise<boolean> {
	if (!hasVideoDecoder()) return false;
	try {
		const support = await VideoDecoder.isConfigSupported({
			codec: c.codec,
			codedWidth: c.width ?? 1280,
			codedHeight: c.height ?? 720,
		});
		return support.supported === true;
	} catch {
		return false;
	}
}

export async function probeVideoEncode(c: VideoCodecCheck): Promise<boolean> {
	if (!hasVideoEncoder()) return false;
	try {
		const support = await VideoEncoder.isConfigSupported({
			codec: c.codec,
			width: c.width ?? 1280,
			height: c.height ?? 720,
			bitrate: c.bitrate ?? 4_000_000,
		});
		return support.supported === true;
	} catch {
		return false;
	}
}

export async function probeAudioDecode(c: AudioCodecCheck): Promise<boolean> {
	if (!hasAudioDecoder()) return false;
	try {
		const support = await AudioDecoder.isConfigSupported({
			codec: c.codec,
			sampleRate: c.sampleRate ?? 48_000,
			numberOfChannels: c.numberOfChannels ?? 2,
		});
		return support.supported === true;
	} catch {
		return false;
	}
}

export async function probeAudioEncode(c: AudioCodecCheck): Promise<boolean> {
	if (!hasAudioEncoder()) return false;
	try {
		const support = await AudioEncoder.isConfigSupported({
			codec: c.codec,
			sampleRate: c.sampleRate ?? 48_000,
			numberOfChannels: c.numberOfChannels ?? 2,
			bitrate: c.bitrate ?? 128_000,
		});
		return support.supported === true;
	} catch {
		return false;
	}
}

//  Tool-level evaluation -

/**
 * Resolve a tool's requirements to one answer for the UI. Container-tier tools
 * always work (no codec). Decode/encode tools probe each declared codec; the
 * first unmet requirement produces the reason string shown in the banner.
 */
export async function evaluateTool(req: ToolRequirements): Promise<CapabilityStatus> {
	if (req.tier === "container") return { supported: true };

	if (!hasWorkers()) {
		return { supported: false, reason: "This browser can't run the background worker the tools need." };
	}

	if (req.videoDecode && !(await probeVideoDecode(req.videoDecode))) {
		return {
			supported: false,
			reason: "Your browser can't decode this video format. Chrome or Edge will work.",
		};
	}
	if (req.audioDecode && !(await probeAudioDecode(req.audioDecode))) {
		return {
			supported: false,
			reason: "Your browser can't decode this audio. Chrome or Edge will work.",
		};
	}
	if (req.videoEncode && !(await probeVideoEncode(req.videoEncode))) {
		return {
			supported: false,
			reason:
				"This tool needs video encoding, which your browser doesn't support yet. It works in Chrome and Edge.",
		};
	}
	if (req.audioEncode && !(await probeAudioEncode(req.audioEncode))) {
		return {
			supported: false,
			reason: "This tool needs audio encoding, which isn't supported in your browser. Try Chrome or Edge.",
		};
	}
	return { supported: true };
}
