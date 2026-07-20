/**
 * Open a user-supplied file as a MediaBunny `Input`. The returned `Input` owns
 * the underlying source; callers are responsible for calling `input.dispose()`
 * when done.
 *
 * Contract (REQUIREMENTS.md §5):
 * - This is the ONLY way consumers should construct an `Input`. Direct
 *   `mediabunny` imports in consumer code are forbidden.
 * - The returned `Input` must be `dispose()`-d by the caller; no leak.
 */

import { ALL_FORMATS, BlobSource, Input } from 'mediabunny';
import { ConvertError } from './protocol';

/** A file-like thing a user hands us: a `File` (browser/web) or a `Blob`.
 *  String URLs are intentionally not supported yet — open the URL and pass
 *  the resulting `Blob` instead. */
export type MediaSource = File | Blob;

/**
 * Open `source` as a MediaBunny `Input`. Throws `ConvertError` with code
 * `bad-input` if the file can't be parsed by any supported demuxer.
 */
export async function openInput(source: MediaSource): Promise<Input> {
	const input = new Input({
		source: new BlobSource(source as Blob),
		formats: ALL_FORMATS,
	});
	try {
		if (!(await input.canRead())) {
			throw new ConvertError('bad-input', "Couldn't read this file. Try MP4, MOV, or WebM.");
		}
	} catch (err) {
		input.dispose();
		if (err instanceof ConvertError) throw err;
		throw new ConvertError(
			'bad-input',
			"This file isn't a supported video. Try MP4, MOV, or WebM.",
		);
	}
	return input;
}
