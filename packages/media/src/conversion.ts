/**
 * Conversion helpers shared by the web app's conversion tools (trim / mute /
 * compress / resize / transcode / extract-audio). Wraps MediaBunny's
 * `Conversion` API and adds progress + cancellation plumbing + consistent
 * error mapping.
 *
 * Contract (REQUIREMENTS.md §5):
 * - Errors are always `ConvertError` (never `Error`). Cancellation is a
 *   `ConvertError` with `code: 'cancelled'`, not a string match.
 * - Progress is reported in [0, 1] on the supplied `JobContext.onProgress`.
 * - The caller owns the `Input`; this module never disposes it.
 */

import {
	BufferTarget,
	Conversion,
	type ConversionAudioOptions,
	type ConversionVideoOptions,
	type Input,
	Mp4OutputFormat,
	Output,
	type OutputFormat,
	WebMOutputFormat,
} from "mediabunny";
import { ConvertError, type JobContext } from "./protocol";

/** Container family the input file belongs to. */
export type ContainerKind = "mp4" | "webm";

/**
 * Container-agnostic options for one conversion. Both video and audio are
 * optional; an op that only mutates video (e.g. compress) leaves audio
 * unset.
 */
export interface ConversionParams {
	outputFormat: OutputFormat;
	video?: ConversionVideoOptions;
	audio?: ConversionAudioOptions;
	/** Trim range in seconds. Omit for whole-file conversion. */
	trim?: { start?: number; end?: number };
}

/**
 * Run a MediaBunny `Conversion` end-to-end against `input`. Returns the
 * final bytes. The caller owns `input`; we never dispose it.
 */
export async function runConversion(
	input: Input,
	params: ConversionParams,
	ctx: JobContext,
): Promise<ArrayBuffer> {
	const target = new BufferTarget();
	const output = new Output({ format: params.outputFormat, target });

	const conversion = await Conversion.init({
		input,
		output,
		video: params.video,
		audio: params.audio,
		trim: params.trim,
		showWarnings: false,
	});
	if (!conversion.isValid) {
		throw new ConvertError(
			"bad-input",
			"This file can't be converted with these settings (no usable track).",
		);
	}
	conversion.onProgress = (p) => ctx.onProgress(p);

	const onAbort = () => void conversion.cancel();
	ctx.signal.addEventListener("abort", onAbort, { once: true });
	try {
		await conversion.execute();
	} catch (err) {
		if (ctx.signal.aborted) throw new ConvertError("cancelled", "Cancelled.");
		throw new ConvertError("bad-input", err instanceof Error ? err.message : "Conversion failed.");
	} finally {
		ctx.signal.removeEventListener("abort", onAbort);
	}

	if (!target.buffer) throw new ConvertError("internal", "No output was produced.");
	return target.buffer;
}

/**
 * Map a high-level `ContainerKind` to a concrete MediaBunny output format.
 * The caller picks the format; this helper just translates.
 */
export function outputFormatFor(kind: ContainerKind): OutputFormat {
	return kind === "webm" ? new WebMOutputFormat() : new Mp4OutputFormat();
}

/**
 * Best-effort guess at the input's container family. Used by "keep the same
 * format" ops (trim, mute) so they don't force a needless transcode.
 */
export async function inputContainerKind(input: Input): Promise<ContainerKind> {
	try {
		const mime = await input.getMimeType();
		return /webm|matroska|x-matroska/i.test(mime) ? "webm" : "mp4";
	} catch {
		return "mp4";
	}
}

/** Replace `name`'s extension with `ext`. */
export function withExtension(name: string, ext: string): string {
	const dot = name.lastIndexOf(".");
	const base = dot > 0 ? name.slice(0, dot) : name;
	return `${base}.${ext}`;
}
