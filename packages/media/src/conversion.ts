/**
 * Conversion helpers shared by the web app's conversion tools (trim / mute /
 * compress / resize / transcode / extract-audio). Wraps MediaBunny's
 * `Conversion` API and adds progress + cancellation plumbing + consistent
 * error mapping. (PR-B will move the apps/web implementation here unchanged.)
 *
 * Contract (REQUIREMENTS.md §5):
 * - Errors are always `MediaError` (never `Error`). Cancellation is a
 *   `MediaError` with `code: 'cancelled'`, not a string match.
 * - Progress is reported in [0, 1] on the supplied `JobContext.onProgress`.
 */

import type {
	ConversionAudioOptions,
	ConversionVideoOptions,
	Input,
	OutputFormat,
} from 'mediabunny';
import type { JobContext } from './protocol';

/** Container family the input file belongs to. */
export type ContainerKind = 'mp4' | 'webm';

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
 *
 * Note: real implementation lands in PR-B. The stub preserves the signature.
 */
export async function runConversion(
	_input: Input,
	_params: ConversionParams,
	_ctx: JobContext,
): Promise<ArrayBuffer> {
	throw new Error('runConversion is not yet implemented — lands in PR-B');
}

/**
 * Map a high-level `ContainerKind` to a concrete MediaBunny output format.
 * The caller picks the format; this helper just translates.
 */
export function outputFormatFor(_kind: ContainerKind): OutputFormat {
	throw new Error('outputFormatFor is not yet implemented — lands in PR-B');
}

/**
 * Best-effort guess at the input's container family. Used by "keep the same
 * format" ops (trim, mute) so they don't force a needless transcode.
 */
export async function inputContainerKind(_input: Input): Promise<ContainerKind> {
	throw new Error('inputContainerKind is not yet implemented — lands in PR-B');
}

/** Replace `name`'s extension with `ext`. */
export function withExtension(name: string, ext: string): string {
	const dot = name.lastIndexOf('.');
	const base = dot > 0 ? name.slice(0, dot) : name;
	return `${base}.${ext}`;
}
