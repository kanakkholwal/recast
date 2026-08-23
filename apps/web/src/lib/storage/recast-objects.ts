import { deleteObject } from "$lib/storage";

/** Absolute URLs are external (legacy/imported); only bare keys are ours. */
const isOwnKey = (v: string | null | undefined): v is string =>
	Boolean(v) && !/^https?:\/\//.test(v as string);

/**
 * Best-effort removal of the objects a recast owns. Call it *after* the
 * database transaction commits: an orphaned object is recoverable from the
 * storage console, a row pointing at a deleted object is not.
 *
 * Failures are logged with the key and swallowed, including the provider 404
 * an already-blobless archived row produces.
 */
export async function deleteRecastObjects(
	recastId: string,
	keys: (string | null | undefined)[],
): Promise<void> {
	const own = [...new Set(keys.filter(isOwnKey))];
	await Promise.all(
		own.map((key) =>
			deleteObject(key).catch((err) => {
				console.error(`[recasts] blob delete failed (recast=${recastId} key=${key})`, err);
			}),
		),
	);
}
