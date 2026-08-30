import { error, json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { z } from "zod";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { recast } from "$lib/db/schema";
import {
	captionsObjectKey,
	deleteObject,
	isStorageConfigured,
	posterObjectKey,
	statObject,
} from "$lib/storage";
import { bumpUsageOnUpload, getQuotaSnapshot } from "$lib/storage/quota";
import type { RequestHandler } from "./$types";

type SessionShape = { user: { id: string } };

const BodySchema = z.object({
	recastId: z.string().min(1),
	width: z.number().int().positive().optional(),
	height: z.number().int().positive().optional(),
	fps: z.number().int().positive().max(240).optional(),
	durationSec: z
		.number()
		.int()
		.nonnegative()
		.max(24 * 60 * 60)
		.optional(),
	/** Client PUT a poster WebP to the signed URL from /init. */
	hasPoster: z.boolean().optional(),
	/** Client PUT a captions VTT to the signed URL from /init. */
	hasCaptions: z.boolean().optional(),
});

/**
 * POST /api/uploads/complete
 *
 * Finalizes a draft recast after the client has PUT the video to the
 * signed URL returned by /api/uploads/init.
 *
 * Trust model: client-supplied byte counts are not trusted — we HEAD the
 * object in R2 and use the server-reported `Content-Length` as the
 * authoritative size. If the object is missing the call returns 410 so
 * the client can retry the PUT.
 *
 * Idempotency: re-running on a recast that's already `published` is a
 * no-op (200 OK with the existing row). This matters because the desktop
 * may retry on flaky networks after a successful upload + dropped reply.
 */
export const POST: RequestHandler = async ({ request }) => {
	if (!isStorageConfigured()) error(503, "Cloud uploads are not configured");

	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;
	if (!session?.user) error(401, "Sign in required");

	let raw: unknown;
	try {
		raw = await request.json();
	} catch {
		error(400, "Invalid JSON body");
	}
	const parsed = BodySchema.safeParse(raw);
	if (!parsed.success) {
		error(400, parsed.error.issues[0]?.message ?? "Invalid body");
	}
	const body = parsed.data;

	const db = getDb();

	const [row] = await db
		.select({
			id: recast.id,
			ownerId: recast.ownerId,
			workspaceId: recast.workspaceId,
			status: recast.status,
			videoUrl: recast.videoUrl,
			sizeBytes: recast.sizeBytes,
		})
		.from(recast)
		.where(eq(recast.id, body.recastId))
		.limit(1);
	if (!row) error(404, "Recast not found");
	if (row.ownerId !== session.user.id) error(403, "Not the owner");

	// Idempotent reply: a retried /complete after success is fine.
	if (row.status === "published") {
		return json({
			ok: true,
			recastId: row.id,
			sizeBytes: row.sizeBytes,
			alreadyComplete: true,
		});
	}

	const stat = await statObject(row.videoUrl);
	if (!stat) {
		// Object isn't in R2: the PUT never finished or hit the wrong key, so the caller should retry.
		return json({ ok: false, reason: "upload_missing" }, { status: 410 });
	}

	const actualBytes = stat.contentLength;

	// A 0-byte upload is an aborted PUT that still returned 2xx; clean up and 422 so the client treats it as a failure.
	if (actualBytes === 0) {
		await deleteObject(row.videoUrl).catch(() => undefined);
		await db.delete(recast).where(eq(recast.id, row.id));
		return json({ ok: false, reason: "empty_upload" }, { status: 422 });
	}

	// Re-check quota with the ACTUAL size: the init pre-check trusted the client's declared size.
	const snapshot = await getQuotaSnapshot(row.workspaceId);
	if (snapshot) {
		// Resolution backstop mirroring the init gate, so a client that lied about `height` can't slip an over-cap file through.
		if (
			body.height != null &&
			Number.isFinite(snapshot.limits.playbackMaxHeight) &&
			body.height > snapshot.limits.playbackMaxHeight + 8
		) {
			await deleteObject(row.videoUrl).catch(() => undefined);
			await db.delete(recast).where(eq(recast.id, row.id));
			return json(
				{
					ok: false,
					denial: {
						reason: "resolution_over_cap",
						heightPx: body.height,
						capHeight: snapshot.limits.playbackMaxHeight,
					},
				},
				{ status: 402 },
			);
		}

		const projected = snapshot.usage.storageBytes + actualBytes;
		if (projected > snapshot.limits.storageBytes) {
			await deleteObject(row.videoUrl).catch(() => undefined);
			await db.delete(recast).where(eq(recast.id, row.id));
			return json(
				{
					ok: false,
					denial: {
						reason: "storage_over_cap",
						currentBytes: snapshot.usage.storageBytes,
						requestedBytes: actualBytes,
						capBytes: snapshot.limits.storageBytes,
					},
				},
				{ status: 402 },
			);
		}
	}

	// Persist the bare object key and sign on read, like the video: a stored public-CDN URL breaks the moment that CDN isn't fronting the bucket.
	const posterUrl: string | undefined = body.hasPoster
		? posterObjectKey(row.workspaceId, row.id)
		: undefined;

	// Persist the bare key, signed on read, so the player can load it as a track; only when the client uploaded one.
	const captionsUrl = body.hasCaptions ? captionsObjectKey(row.workspaceId, row.id) : undefined;

	await db.transaction(async (tx) => {
		await tx
			.update(recast)
			.set({
				sizeBytes: actualBytes,
				width: body.width,
				height: body.height,
				fps: body.fps,
				durationSec: body.durationSec ?? undefined,
				posterUrl,
				captionsUrl,
				status: "published",
				updatedAt: new Date(),
			})
			.where(eq(recast.id, row.id));
		await bumpUsageOnUpload(row.workspaceId, actualBytes, tx);
	});

	return json({
		ok: true,
		recastId: row.id,
		sizeBytes: actualBytes,
	});
};
