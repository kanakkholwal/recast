import { error, json } from "@sveltejs/kit";
import { and, eq, like, ne, sql } from "drizzle-orm";
import { getDb } from "$lib/db";
import { folder } from "$lib/db/schema";
import { assertWorkspaceMember, requireUser } from "$lib/workspace/guard";
import type { RequestHandler } from "./$types";

const MAX_NAME = 80;
const HEX = /^#[0-9a-fA-F]{6}$/;

function cleanColor(v: unknown): string | null {
	return typeof v === "string" && HEX.test(v.trim()) ? v.trim() : null;
}

async function loadFolder(id: string) {
	const db = getDb();
	const [f] = await db
		.select({
			id: folder.id,
			workspaceId: folder.workspaceId,
			parentId: folder.parentId,
			path: folder.path,
		})
		.from(folder)
		.where(eq(folder.id, id))
		.limit(1);
	return f ?? null;
}

/**
 * PATCH /api/folders/[id] — rename / recolor / move.
 * Body (only provided keys applied):
 *   - name     : 1–80 chars
 *   - color    : "#rrggbb" or null
 *   - parentId : a folder id in the same workspace, or null for root (move)
 *
 * Paths are ID-based, so a rename leaves `path` untouched; a move rewrites
 * this folder's `path` AND every descendant's (prefix swap), guarding
 * against moving a folder into its own subtree.
 */
export const PATCH: RequestHandler = async ({ params, request }) => {
	const u = await requireUser(request);
	const f = await loadFolder(params.id);
	if (!f) error(404, "Folder not found");
	await assertWorkspaceMember(u.id, f.workspaceId);

	let body: { name?: unknown; color?: unknown; parentId?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const self: {
		name?: string;
		color?: string | null;
		parentId?: string | null;
		path?: string;
		updatedAt: Date;
	} = {
		updatedAt: new Date(),
	};

	if ("name" in body) {
		const name = typeof body.name === "string" ? body.name.trim().slice(0, MAX_NAME) : "";
		if (!name) error(400, "Folder name can't be empty");
		self.name = name;
	}
	if ("color" in body) self.color = cleanColor(body.color);

	const db = getDb();

	// Move: resolve the destination parent + new path, with cycle protection.
	let moved = false;
	const oldPath = f.path;
	let newPath = f.path;
	if ("parentId" in body) {
		const newParentId = typeof body.parentId === "string" && body.parentId ? body.parentId : null;
		if (newParentId === f.id) error(400, "A folder can't be its own parent");

		let newParentPath = "/";
		if (newParentId) {
			const [parent] = await db
				.select({ path: folder.path })
				.from(folder)
				.where(and(eq(folder.id, newParentId), eq(folder.workspaceId, f.workspaceId)))
				.limit(1);
			if (!parent) error(404, "Destination folder not found in this workspace");
			// Moving into own subtree would orphan the tree — reject.
			if (parent.path.startsWith(f.path)) {
				error(400, "Can't move a folder into itself");
			}
			newParentPath = parent.path;
		}
		newPath = `${newParentPath}${f.id}/`;
		if (newPath !== oldPath) {
			moved = true;
			self.parentId = newParentId;
			self.path = newPath;
		}
	}

	if (!("name" in self) && !("color" in self) && !("parentId" in self)) {
		error(400, "Nothing to update");
	}

	try {
		await db.transaction(async (tx) => {
			if (moved) {
				// Rewrite descendants first by prefix swap, excluding self, which gets its new parentId below.
				await tx
					.update(folder)
					.set({
						path: sql`${newPath} || substring(${folder.path} from ${oldPath.length + 1})`,
						updatedAt: new Date(),
					})
					.where(
						and(
							eq(folder.workspaceId, f.workspaceId),
							like(folder.path, `${oldPath}%`),
							ne(folder.id, f.id),
						),
					);
			}
			await tx.update(folder).set(self).where(eq(folder.id, f.id));
		});
	} catch (e) {
		const msg = String((e as Error)?.message ?? e);
		if (msg.includes("folder_parent_name_key")) {
			error(409, "A folder with that name already exists here");
		}
		throw e;
	}

	return json({
		ok: true,
		...(self.name !== undefined ? { name: self.name } : {}),
		...("color" in self ? { color: self.color } : {}),
		...(moved ? { parentId: self.parentId, path: self.path } : {}),
	});
};

/**
 * DELETE /api/folders/[id] — delete the folder and its entire subtree.
 * Recasts inside any deleted folder fall back to root (`recast.folderId`
 * SET NULL via FK). Local `.recast` files and cloud blobs are untouched.
 */
export const DELETE: RequestHandler = async ({ params, request }) => {
	const u = await requireUser(request);
	const f = await loadFolder(params.id);
	if (!f) error(404, "Folder not found");
	await assertWorkspaceMember(u.id, f.workspaceId);

	const db = getDb();
	// Subtree incl. self: every folder whose path begins with this one's.
	await db
		.delete(folder)
		.where(and(eq(folder.workspaceId, f.workspaceId), like(folder.path, `${f.path}%`)));

	return json({ ok: true });
};
