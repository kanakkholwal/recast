/**
 * Small output encoders for the formats MediaBunny / WebCodecs don't write
 * directly: GIF (gifenc), WAV (raw PCM, no library), MP3 (lamejs), and ZIP
 * (fflate) for multi-file output like extracted frames. Each is a thin, pure
 * helper the handlers drive frame-by-frame or buffer-by-buffer.
 */

import { Mp3Encoder } from '@breezystack/lamejs';
import { zipSync } from 'fflate';
import { applyPalette, GIFEncoder, quantize } from 'gifenc';

//  GIF -

/** A GIF writer backed by gifenc. Quantizes each frame to a 256-colour palette. */
export interface GifWriter {
	/** Add one frame from RGBA pixels; `delayMs` is how long it shows. */
	addFrame(
		rgba: Uint8Array | Uint8ClampedArray,
		width: number,
		height: number,
		delayMs: number,
	): void;
	/** Finish and return the GIF bytes. */
	finish(): Uint8Array;
}

/** A GIF writer backed by gifenc. Quantizes each frame to a 256-colour palette. */
export function createGifWriter(): GifWriter {
	const enc = GIFEncoder();
	return {
		addFrame(rgba, width, height, delayMs) {
			const data = rgba instanceof Uint8Array ? rgba : new Uint8Array(rgba.buffer);
			const palette = quantize(data, 256);
			const index = applyPalette(data, palette);
			enc.writeFrame(index, width, height, {
				palette,
				delay: Math.max(20, Math.round(delayMs)),
			});
		},
		finish() {
			enc.finish();
			return enc.bytes();
		},
	};
}

//  PCM helpers --

/** Interleave planar float channels into one Int16 PCM buffer. */
function interleaveToInt16(channels: Float32Array[], frames: number): Int16Array {
	const numChannels = channels.length;
	const out = new Int16Array(frames * numChannels);
	for (let i = 0; i < frames; i++) {
		for (let c = 0; c < numChannels; c++) {
			const s = Math.max(-1, Math.min(1, channels[c][i] ?? 0));
			out[i * numChannels + c] = s < 0 ? s * 0x8000 : s * 0x7fff;
		}
	}
	return out;
}

//  WAV (16-bit PCM, no library) --

/** Encode planar float channels to a 16-bit PCM WAV file. */
export function encodeWav(channels: Float32Array[], sampleRate: number): Uint8Array {
	const numChannels = Math.max(1, channels.length);
	const frames = channels[0]?.length ?? 0;
	const pcm = interleaveToInt16(channels, frames);
	const dataBytes = pcm.length * 2;
	const buffer = new ArrayBuffer(44 + dataBytes);
	const view = new DataView(buffer);
	const writeStr = (off: number, s: string) => {
		for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i));
	};
	const byteRate = sampleRate * numChannels * 2;
	writeStr(0, 'RIFF');
	view.setUint32(4, 36 + dataBytes, true);
	writeStr(8, 'WAVE');
	writeStr(12, 'fmt ');
	view.setUint32(16, 16, true); // PCM chunk size
	view.setUint16(20, 1, true); // PCM
	view.setUint16(22, numChannels, true);
	view.setUint32(24, sampleRate, true);
	view.setUint32(28, byteRate, true);
	view.setUint16(32, numChannels * 2, true); // block align
	view.setUint16(34, 16, true); // bits per sample
	writeStr(36, 'data');
	view.setUint32(40, dataBytes, true);
	new Int16Array(buffer, 44).set(pcm);
	return new Uint8Array(buffer);
}

//  MP3 (lamejs) -

/** Encode planar float channels to MP3 (mono or stereo). */
export function encodeMp3(channels: Float32Array[], sampleRate: number, kbps = 192): Uint8Array {
	const numChannels = channels.length >= 2 ? 2 : 1;
	const encoder = new Mp3Encoder(numChannels, sampleRate, kbps);
	const frames = channels[0]?.length ?? 0;
	const left = floatToInt16(channels[0] ?? new Float32Array(0));
	const right = numChannels === 2 ? floatToInt16(channels[1]) : left;

	const chunk = 1152; // MP3 frame size
	const parts: Uint8Array[] = [];
	for (let i = 0; i < frames; i += chunk) {
		const l = left.subarray(i, i + chunk);
		const r = right.subarray(i, i + chunk);
		const buf = numChannels === 2 ? encoder.encodeBuffer(l, r) : encoder.encodeBuffer(l);
		if (buf.length > 0) parts.push(new Uint8Array(buf));
	}
	const end = encoder.flush();
	if (end.length > 0) parts.push(new Uint8Array(end));
	return concat(parts);
}

function floatToInt16(f: Float32Array): Int16Array {
	const out = new Int16Array(f.length);
	for (let i = 0; i < f.length; i++) {
		const s = Math.max(-1, Math.min(1, f[i]));
		out[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
	}
	return out;
}

function concat(parts: Uint8Array[]): Uint8Array {
	let total = 0;
	for (const p of parts) total += p.length;
	const out = new Uint8Array(total);
	let off = 0;
	for (const p of parts) {
		out.set(p, off);
		off += p.length;
	}
	return out;
}

//  ZIP (fflate) -

/** Zip a set of already-compressed files (images) with no extra compression. */
export function zipFiles(files: Record<string, Uint8Array>): Uint8Array {
	return zipSync(files, { level: 0 });
}
