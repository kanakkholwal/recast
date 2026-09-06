import { error, json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import type { SignedUpload } from "files-sdk";
import { getDb } from "$lib/db";
import { recast } from "$lib/db/schema";
import { authorizeRecast } from "$lib/server/recast-guard";
import {
	deleteObject,
	isStorageConfigured,
	objectKeyFromStored,
	posterObjectKey,
	resolvePlaybackUrl,
	signUploadUrl,
	statObject,
} from "$lib/storage";
import type { RequestHandler } from "./$types";

/**
 * Owner-or-admin gate for poster mutations. Returns the fields the
 * replace flow needs (workspace for the key, the current poster so we can
 * delete it afterward). Mirrors the gate in `../+server.ts` — kept inline,
 * matching that file's own DELETE handler.
 */

/** Short cache-busting token for the new poster key. */
function newVersion(): string {
	return crypto.randomUUID().replace(/-/g, "").slice(0, 10);
}

/**
 * POST /api/recasts/[id]/poster
 *
 * Step 1 of poster replacement: authorize, then return a signed PUT URL bound
 * to a fresh versioned key (`{recastId}.poster.{version}.webp`). The client
 * PUTs a WebP frame there, then calls PUT below to finalize.
 */
export const POST: RequestHandler = async ({ params, request }) => {
	if (!isStorageConfigured()) error(503, "Cloud storage is not configured");
	const row = await authorizeRecast(request, params.id);

	const version = newVersion();
	const key = posterObjectKey(row.workspaceId, row.id, version);

	let upload: SignedUpload;
	try {
		upload = await signUploadUrl({ key, contentType: "image/webp" });
	} catch (err) {
		console.error("[recasts/poster] sign failed", err);
		error(500, "Could not generate upload URL");
	}

	return json({ ok: true, version, key, upload, expiresInSeconds: 15 * 60 });
};

/**
 * PUT /api/recasts/[id]/poster
 *
 * Step 2: HEAD-verify the freshly uploaded poster landed, point the recast at
 * it, and delete the previous poster blob (best-effort — orphaning an image is
 * recoverable, stranding the row on a missing poster isn't). Body: { version }.
 */
export const PUT: RequestHandler = async ({ params, request }) => {
	if (!isStorageConfigured()) error(503, "Cloud storage is not configured");
	const row = await authorizeRecast(request, params.id);

	let body: { version?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}
	const version =
		typeof body.version === "string" ? body.version.replace(/[^a-z0-9]/gi, "").slice(0, 32) : "";
	if (!version) error(400, "Missing poster version");

	const key = posterObjectKey(row.workspaceId, row.id, version);

	const stat = await statObject(key);
	if (!stat || stat.contentLength === 0) {
		// The PUT never landed, so clean up the stray empty object and tell the client to retry.
		if (stat) await deleteObject(key).catch(() => undefined);
		return json({ ok: false, reason: "upload_missing" }, { status: 410 });
	}

	// Persist the bare key and sign on read, so the cover works whether or not a public CDN is serving.
	const posterUrl = key;
	const db = getDb();
	await db.update(recast).set({ posterUrl, updatedAt: new Date() }).where(eq(recast.id, row.id));

	// Skip when it is the same key or an external legacy URL we don't own.
	const oldKey = objectKeyFromStored(row.posterUrl);
	if (oldKey && oldKey !== key) {
		await deleteObject(oldKey).catch((err) => {
			console.error(
				`[recasts/poster] old poster delete failed for ${row.id} (key=${oldKey}) — new poster still set`,
				err,
			);
		});
	}

	// Return a displayable URL so the client can swap the thumbnail without a full reload.
	const displayUrl = await resolvePlaybackUrl(posterUrl);
	return json({ ok: true, posterUrl: displayUrl });
};
