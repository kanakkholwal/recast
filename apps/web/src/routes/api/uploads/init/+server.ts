import { error, json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { z } from "zod";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { recast } from "$lib/db/schema";
import {
	captionsObjectKey,
	isStorageConfigured,
	posterObjectKey,
	recastObjectKey,
	signUploadUrl,
} from "$lib/storage";
import { checkUploadAllowed, getQuotaSnapshot, type UploadDenial } from "$lib/storage/quota";
import { assertWorkspaceMember } from "$lib/workspace/guard";
import type { RequestHandler } from "./$types";

type SessionShape = {
	user: { id: string; activeOrganizationId?: string | null };
};

const BodySchema = z.object({
	workspaceId: z.string().min(1).optional(),
	title: z.string().trim().min(1).max(200),
	durationSec: z
		.number()
		.int()
		.nonnegative()
		.max(24 * 60 * 60),
	sizeBytes: z.number().int().nonnegative(),
	width: z.number().int().positive().optional(),
	height: z.number().int().positive().optional(),
	fps: z.number().int().positive().max(240).optional(),
	/** Video MIME — signs the PUT so the bytes match. mp4 or webm only. */
	contentType: z.enum(["video/mp4", "video/webm"]).default("video/mp4"),
	/** Client has a caption track to upload → sign a captions PUT URL too. */
	hasCaptions: z.boolean().optional(),
});

/**
 * POST /api/uploads/init
 *
 * Reserves a draft recast row and returns a 15-minute pre-signed PUT URL
 * for the client to upload the video to R2.
 *
 * Flow:
 *   1. Auth: signed-in user only.
 *   2. Resolve workspace: explicit `workspaceId` in body, or the session's
 *      `activeOrganizationId` as fallback.
 *   3. Verify membership in that workspace.
 *   4. Quota gate: workspace must be under its plan's storage / count /
 *      duration caps.
 *   5. Insert a `recast` row with status='draft', sizeBytes still 0 (we
 *      learn the real size on /complete via R2 HEAD).
 *   6. Sign and return the PUT URL bound to the client's `Content-Type`
 *      (`video/mp4` or `video/webm`).
 *
 * On any failure after step 5 (e.g. signing fails) the draft row is rolled
 * back so a 5xx doesn't leak placeholder rows into the user's library.
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

	const workspaceId = body.workspaceId ?? session.user.activeOrganizationId;
	if (!workspaceId) error(400, "No active workspace");

	const db = getDb();

	// Don't trust the session's activeOrganizationId alone: a stale pointer must fail closed, not upload into a left workspace.
	await assertWorkspaceMember(session.user.id, workspaceId);

	const snapshot = await getQuotaSnapshot(workspaceId);
	if (!snapshot) error(404, "Workspace not found");

	const gate = checkUploadAllowed(snapshot, {
		sizeBytes: body.sizeBytes,
		durationSec: body.durationSec,
		heightPx: body.height,
	});
	if (!gate.ok) {
		return json({ ok: false, denial: gate.denial }, { status: denialStatus(gate.denial) });
	}

	const recastId = crypto.randomUUID();
	const key = recastObjectKey(workspaceId, recastId);

	await db.insert(recast).values({
		id: recastId,
		workspaceId,
		ownerId: session.user.id,
		title: body.title,
		durationSec: body.durationSec,
		sizeBytes: 0,
		width: body.width,
		height: body.height,
		fps: body.fps,
		// Stored as the relative key; absolute URLs are derived at read time so buckets or CDNs can swap without rewriting rows.
		videoUrl: key,
		provider: "r2",
		source: "cloud",
		status: "draft",
	});

	let upload;
	try {
		upload = await signUploadUrl({ key, contentType: body.contentType });
	} catch (err) {
		// Roll back the draft row: leaving it would count against the active-recasts cap for something never uploadable.
		await db.delete(recast).where(eq(recast.id, recastId));
		console.error("[uploads/init] sign failed", err);
		error(500, "Could not generate upload URL");
	}

	// Best-effort and non-fatal: a failure just skips the poster, and the video is the only required asset.
	let posterUpload;
	try {
		posterUpload = await signUploadUrl({
			key: posterObjectKey(workspaceId, recastId),
			contentType: "image/webp",
		});
	} catch (err) {
		console.error("[uploads/init] poster sign failed (non-fatal)", err);
		posterUpload = undefined;
	}

	// Captions track PUT (WebVTT), same best-effort contract as the poster.
	let captionsUpload;
	if (body.hasCaptions) {
		try {
			captionsUpload = await signUploadUrl({
				key: captionsObjectKey(workspaceId, recastId),
				contentType: "text/vtt",
			});
		} catch (err) {
			console.error("[uploads/init] captions sign failed (non-fatal)", err);
			captionsUpload = undefined;
		}
	}

	// `upload` is a discriminated union from files-sdk, and the client picks its fetch shape from `method`.
	return json({
		ok: true,
		recastId,
		key,
		upload,
		posterUpload,
		captionsUpload,
		expiresInSeconds: 15 * 60,
	});
};

function denialStatus(d: UploadDenial): number {
	switch (d.reason) {
		case "workspace_not_found":
			return 404;
		case "duration_over_cap":
		case "resolution_over_cap":
		case "active_recasts_over_cap":
		case "storage_over_cap":
			return 402; // Payment Required — quota / plan gate
	}
}
