/**
 * Browser-side "replace poster" flow, the counterpart to the poster the
 * upload pipeline captures from the first frame. Lets an owner pick any image
 * and set it as the recast's cover. Drives two endpoints:
 *
 *   1. POST /api/recasts/{id}/poster      → signs a PUT to a fresh versioned key
 *   2. PUT  <signed url>                   → uploads the re-encoded WebP
 *   3. PUT  /api/recasts/{id}/poster       → finalizes + deletes the old blob
 *
 * The chosen image is downscaled + re-encoded to WebP client-side so we store a
 * lean, consistent poster regardless of what the user picked (a 12 MP phone
 * photo becomes a ~960px WebP).
 */

import { browser } from "$app/environment";

/** Cap matches the upload-time poster (`pickBestPosterFrame`) so covers are uniform. */
const MAX_POSTER_WIDTH = 960;
const MAX_INPUT_BYTES = 25 * 1024 * 1024;

export const POSTER_ACCEPT = "image/png,image/jpeg,image/webp,image/avif";

export function isPosterImage(file: File): boolean {
	return /^image\/(png|jpe?g|webp|avif)$/i.test(file.type);
}

interface SignedEnvelope {
	method: string;
	url: string;
	headers?: Record<string, string>;
}

/** Decode → downscale → WebP. Returns null if the image can't be read. */
async function reencodeToWebp(file: File): Promise<Blob | null> {
	let bitmap: ImageBitmap;
	try {
		bitmap = await createImageBitmap(file);
	} catch {
		return null;
	}
	try {
		const w = bitmap.width;
		const h = bitmap.height;
		if (!w || !h) return null;
		const scaleW = Math.min(MAX_POSTER_WIDTH, w);
		const scaleH = Math.max(1, Math.round(h * (scaleW / w)));
		const canvas = document.createElement("canvas");
		canvas.width = scaleW;
		canvas.height = scaleH;
		const ctx = canvas.getContext("2d");
		if (!ctx) return null;
		ctx.drawImage(bitmap, 0, 0, scaleW, scaleH);
		return await new Promise<Blob | null>((resolve) =>
			canvas.toBlob((b) => resolve(b), "image/webp", 0.82),
		);
	} finally {
		bitmap.close();
	}
}

async function putBlob(envelope: SignedEnvelope, body: Blob, contentType: string): Promise<void> {
	const headers = envelope.headers ?? {};
	const hasContentType = Object.keys(headers).some((k) => k.toLowerCase() === "content-type");
	const res = await fetch(envelope.url, {
		method: "PUT",
		headers: hasContentType ? headers : { ...headers, "content-type": contentType },
		body,
	});
	if (!res.ok) throw new Error(`Upload rejected (${res.status}).`);
}

async function readMessage(res: Response): Promise<string> {
	try {
		const data = (await res.json()) as { message?: string };
		return data?.message ?? "";
	} catch {
		return "";
	}
}

/**
 * Store a ready WebP blob as `recastId`'s poster (init → PUT → finalize).
 * Used both by "replace cover" (a re-encoded image) and by the upload dialog's
 * frame scrubber (a captured video frame). Resolves to the new poster URL.
 */
export async function uploadPosterBlob(recastId: string, webp: Blob): Promise<string> {
	if (!browser) throw new Error("Poster upload can only run in the browser.");

	// 1. init (sign)
	const initRes = await fetch(`/api/recasts/${recastId}/poster`, { method: "POST" });
	if (!initRes.ok) throw new Error((await readMessage(initRes)) || "Couldn't start the upload.");
	const init = (await initRes.json()) as { version?: string; upload?: SignedEnvelope };
	if (init.upload?.method?.toUpperCase() !== "PUT" || !init.version) {
		throw new Error("This storage provider doesn't support poster replacement yet.");
	}

	// 2. PUT the WebP
	await putBlob(init.upload, webp, "image/webp");

	// 3. finalize (verify + swap + delete old)
	const doneRes = await fetch(`/api/recasts/${recastId}/poster`, {
		method: "PUT",
		headers: { "content-type": "application/json" },
		body: JSON.stringify({ version: init.version }),
	});
	if (!doneRes.ok) {
		throw new Error((await readMessage(doneRes)) || "Couldn't save the new poster.");
	}
	const done = (await doneRes.json()) as { posterUrl?: string };
	return done.posterUrl ?? "";
}

/**
 * Replace `recastId`'s poster with `file`. Resolves to a directly displayable
 * URL for the new poster (use it to update the local store / thumbnail). Throws
 * an Error with a user-facing message on failure.
 */
export async function replacePoster(recastId: string, file: File): Promise<string> {
	if (!browser) throw new Error("Poster replace can only run in the browser.");
	if (!isPosterImage(file)) throw new Error("Pick a PNG, JPEG, WebP, or AVIF image.");
	if (file.size > MAX_INPUT_BYTES) throw new Error("That image is too large (max 25 MB).");

	const webp = await reencodeToWebp(file);
	if (!webp) throw new Error("Couldn't read that image, try another file.");

	return uploadPosterBlob(recastId, webp);
}
