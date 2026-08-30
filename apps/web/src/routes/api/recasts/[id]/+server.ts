import { error, json } from "@sveltejs/kit";
import { and, eq, sql } from "drizzle-orm";
import { getDb } from "$lib/db";
import { folder, recast, workspaceUsage } from "$lib/db/schema";
import { authorizeRecast } from "$lib/server/recast-guard";
import { decrementUsageOnDelete } from "$lib/storage/quota";
import { deleteRecastObjects } from "$lib/storage/recast-objects";
import type { RequestHandler } from "./$types";

const MAX_TITLE = 200;
const MAX_DESCRIPTION = 2000;

/**
 * PATCH /api/recasts/[id]
 *
 * Edit and/or move a recast. Body (only provided keys are written):
 *   - title       : 1–200 chars
 *   - description : up to 2000 chars; empty string clears it (null)
 *   - folderId    : a folder id in the SAME workspace, or null to move to root
 *
 * Creator, workspace owner/admin, or platform admin.
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
			// The folder must exist and belong to the recast's workspace, or a recast could be filed into another workspace's tree.
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
 * Creator, workspace owner/admin, or platform admin. Idempotent-ish: a second
 * call 404s once the
 * row is gone. This is the desktop "delete cloud copy" action: it never
 * touches the local `.recast`, which remains the source of truth.
 */
export const DELETE: RequestHandler = async ({ params, request }) => {
	const row = await authorizeRecast(request, params.id);
	const db = getDb();

	// The row first, objects after: a cascade makes the delete all-or-nothing, and dropping blobs first could strand a live row. Consumed delivery is deliberately not reversed.
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

	// Best-effort, post-commit: a failure orphans an object (recoverable from the storage console) rather than stranding the row.
	await deleteRecastObjects(row.id, [row.videoUrl, row.posterUrl]);

	return json({ ok: true });
};
