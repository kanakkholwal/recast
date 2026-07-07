import { error, json } from "@sveltejs/kit";
import { and, eq, sql } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { folder, recast, user, workspaceUsage } from "$lib/db/schema";
import { decrementUsageOnDelete } from "$lib/storage/quota";
import { deleteObject } from "$lib/storage";
import type { RequestHandler } from "./$types";

type SessionShape = { user: { id: string; role?: string } };

const MAX_TITLE = 200;
const MAX_DESCRIPTION = 2000;

/** Owner-or-admin gate shared by PATCH and DELETE. Returns the recast row. */
async function authorizeRecast(
	request: Request,
	recastId: string,
): Promise<{
	id: string;
	ownerId: string;
	workspaceId: string;
	videoUrl: string;
	sizeBytes: number;
	status: string;
}> {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;
	if (!session?.user) error(401, "Sign in required");

	const db = getDb();
	const [row] = await db
		.select({
			id: recast.id,
			ownerId: recast.ownerId,
			workspaceId: recast.workspaceId,
			videoUrl: recast.videoUrl,
			sizeBytes: recast.sizeBytes,
			status: recast.status,
		})
		.from(recast)
		.where(eq(recast.id, recastId))
		.limit(1);
	if (!row) error(404, "Recast not found");

	const isOwner = row.ownerId === session.user.id;
	if (!isOwner) {
		const [u] = await db
			.select({ role: user.role })
			.from(user)
			.where(eq(user.id, session.user.id))
			.limit(1);
		if (u?.role !== "admin") error(403, "Not allowed to modify this recast");
	}
	return row;
}

/**
 * PATCH /api/recasts/[id]
 *
 * Edit and/or move a recast. Body (only provided keys are written):
 *   - title       : 1–200 chars
 *   - description : up to 2000 chars; empty string clears it (null)
 *   - folderId    : a folder id in the SAME workspace, or null to move to root
 *
 * Owner or global admin only.
 */
export const PATCH: RequestHandler = async ({ params, request }) => {
	const row = await authorizeRecast(request, params.id);

	let body: { title?: unknown; description?: unknown; folderId?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const patch: {
		title?: string;
		description?: string | null;
		folderId?: string | null;
		updatedAt: Date;
	} = {
		updatedAt: new Date(),
	};

	if ("title" in body) {
		const title = typeof body.title === "string" ? body.title.trim().slice(0, MAX_TITLE) : "";
		if (!title) error(400, "Title can't be empty");
		patch.title = title;
	}

	if ("description" in body) {
		const d =
			typeof body.description === "string" ? body.description.trim().slice(0, MAX_DESCRIPTION) : "";
		patch.description = d || null; // empty clears it
	}

	const db = getDb();
	if ("folderId" in body) {
		if (body.folderId === null) {
			patch.folderId = null;
		} else if (typeof body.folderId === "string") {
			// The folder must exist and belong to the recast's workspace —
			// otherwise a user could file a recast into another workspace's tree.
			const [f] = await db
				.select({ id: folder.id })
				.from(folder)
				.where(and(eq(folder.id, body.folderId), eq(folder.workspaceId, row.workspaceId)))
				.limit(1);
			if (!f) error(404, "Folder not found in this workspace");
			patch.folderId = body.folderId;
		} else {
			error(400, "Invalid folderId");
		}
	}

	if (!("title" in patch) && !("description" in patch) && !("folderId" in patch)) {
		error(400, "Nothing to update");
	}

	await db.update(recast).set(patch).where(eq(recast.id, row.id));
	return json({
		ok: true,
		...(patch.title !== undefined ? { title: patch.title } : {}),
		...("description" in patch ? { description: patch.description } : {}),
		...("folderId" in patch ? { folderId: patch.folderId } : {}),
	});
};

/**
 * DELETE /api/recasts/[id]
 *
 * Permanently removes a cloud recast: the R2 blob, the row (its shares,
 * comments, reactions, and views cascade-delete via FK), and the
 * workspace_usage accounting.
 *
 * Usage reversal mirrors the expiry sweep's model:
 *   - `published` → reclaim storage + decrement active count
 *   - `archived`  → blob already gone / size 0; decrement archived count
 *   - `draft`     → never bumped usage; nothing to reverse
 *
 * Owner or global admin only. Idempotent-ish: a second call 404s once the
 * row is gone. This is the desktop "delete cloud copy" action — it never
 * touches the local `.recast`, which remains the source of truth.
 */
export const DELETE: RequestHandler = async ({ params, request }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;
	if (!session?.user) error(401, "Sign in required");

	const db = getDb();

	const [row] = await db
		.select({
			id: recast.id,
			ownerId: recast.ownerId,
			workspaceId: recast.workspaceId,
			videoUrl: recast.videoUrl,
			sizeBytes: recast.sizeBytes,
			status: recast.status,
		})
		.from(recast)
		.where(eq(recast.id, params.id))
		.limit(1);
	if (!row) error(404, "Recast not found");

	// Authorize: owner OR global admin. Re-read the role so a role change
	// takes effect immediately rather than waiting on session re-issue.
	const isOwner = row.ownerId === session.user.id;
	let isAdmin = false;
	if (!isOwner) {
		const [u] = await db
			.select({ role: user.role })
			.from(user)
			.where(eq(user.id, session.user.id))
			.limit(1);
		isAdmin = u?.role === "admin";
	}
	if (!isOwner && !isAdmin) error(403, "Not allowed to delete this recast");

	// Best-effort blob delete. Skip legacy/external absolute URLs (only bare
	// object keys are ours to remove). Archived rows may already be blobless —
	// a 404 from the provider is fine, so swallow errors and still drop the row
	// rather than stranding it. A non-404 failure (e.g. an Azure/S3 auth or
	// firewall 403) is logged with the key but still non-fatal: orphaning the
	// blob is recoverable via the storage console; stranding the DB row isn't.
	if (row.videoUrl && !/^https?:\/\//.test(row.videoUrl)) {
		await deleteObject(row.videoUrl).catch((err) => {
			console.error(
				`[recasts/delete] blob delete failed for ${row.id} (key=${row.videoUrl}) — row still removed`,
				err,
			);
		});
	}

	await db.transaction(async (tx) => {
		await tx.delete(recast).where(eq(recast.id, row.id));
		if (row.status === "published") {
			await decrementUsageOnDelete(row.workspaceId, row.sizeBytes, tx);
		} else if (row.status === "archived") {
			await tx
				.update(workspaceUsage)
				.set({
					archivedRecastsCount: sql`GREATEST(${workspaceUsage.archivedRecastsCount} - 1, 0)`,
					updatedAt: new Date(),
				})
				.where(eq(workspaceUsage.workspaceId, row.workspaceId));
		}
		// `draft` never bumped usage — nothing to reverse.
	});

	return json({ ok: true });
};
