import { error, json } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { folder } from "$lib/db/schema";
import { assertWorkspaceMember, requireUser } from "$lib/workspace/guard";
import type { RequestHandler } from "./$types";

const MAX_NAME = 80;
const HEX = /^#[0-9a-fA-F]{6}$/;

function cleanColor(v: unknown): string | null {
	return typeof v === "string" && HEX.test(v.trim()) ? v.trim() : null;
}

/**
 * Folders are nested via `parentId` + a materialized `path`. We build the
 * path from IDs ("/<id>/<id>/") rather than names, so a rename never has to
 * rewrite descendants — only a move does. `%`-prefix queries on `path` give
 * cheap subtree reads without recursive CTEs.
 */

/** GET /api/folders?workspaceId=… — all folders in a workspace (flat; the UI builds the tree). */
export const GET: RequestHandler = async ({ request, url }) => {
	const u = await requireUser(request);
	const workspaceId = url.searchParams.get("workspaceId");
	if (!workspaceId) error(400, "workspaceId is required");
	await assertWorkspaceMember(u.id, workspaceId);

	const db = getDb();
	const rows = await db
		.select({
			id: folder.id,
			parentId: folder.parentId,
			name: folder.name,
			color: folder.color,
			path: folder.path,
		})
		.from(folder)
		.where(eq(folder.workspaceId, workspaceId));

	return json({ ok: true, folders: rows });
};

/** POST /api/folders — create a folder. Body: { workspaceId, name, parentId?, color? } */
export const POST: RequestHandler = async ({ request }) => {
	const u = await requireUser(request);

	let body: {
		workspaceId?: unknown;
		name?: unknown;
		parentId?: unknown;
		color?: unknown;
	} = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const workspaceId = typeof body.workspaceId === "string" ? body.workspaceId : "";
	const name = typeof body.name === "string" ? body.name.trim().slice(0, MAX_NAME) : "";
	const parentId = typeof body.parentId === "string" && body.parentId ? body.parentId : null;
	if (!workspaceId) error(400, "workspaceId is required");
	if (!name) error(400, "Folder name is required");

	await assertWorkspaceMember(u.id, workspaceId);

	const db = getDb();

	// Resolve the parent's path (and confirm it's in the same workspace).
	let parentPath = "/";
	if (parentId) {
		const [parent] = await db
			.select({ path: folder.path })
			.from(folder)
			.where(and(eq(folder.id, parentId), eq(folder.workspaceId, workspaceId)))
			.limit(1);
		if (!parent) error(404, "Parent folder not found in this workspace");
		parentPath = parent.path;
	}

	const id = crypto.randomUUID();
	const path = `${parentPath}${id}/`;

	try {
		await db.insert(folder).values({
			id,
			workspaceId,
			parentId,
			name,
			color: cleanColor(body.color),
			path,
			createdBy: u.id,
		});
	} catch (e) {
		// Unique (workspace, parentId, name) — a sibling already has this name.
		const msg = (e as Error)?.message ?? String(e);
		if (msg.includes("folder_parent_name_key")) {
			error(409, "A folder with that name already exists here");
		}
		throw e;
	}

	return json(
		{ ok: true, folder: { id, parentId, name, color: cleanColor(body.color), path } },
		{ status: 201 },
	);
};
