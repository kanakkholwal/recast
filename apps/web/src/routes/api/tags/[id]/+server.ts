import { error, json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { tag } from "$lib/db/schema";
import { assertWorkspaceMember, requireUser } from "$lib/workspace/guard";
import type { RequestHandler } from "./$types";

const MAX_NAME = 40;
const HEX = /^#[0-9a-fA-F]{6}$/;

function cleanColor(v: unknown): string | null {
	return typeof v === "string" && HEX.test(v.trim()) ? v.trim() : null;
}

async function loadTag(id: string) {
	const db = getDb();
	const [t] = await db
		.select({ id: tag.id, workspaceId: tag.workspaceId })
		.from(tag)
		.where(eq(tag.id, id))
		.limit(1);
	return t ?? null;
}

/** PATCH /api/tags/[id] — rename / recolor. Body: { name?, color? } */
export const PATCH: RequestHandler = async ({ params, request }) => {
	const u = await requireUser(request);
	const t = await loadTag(params.id);
	if (!t) error(404, "Tag not found");
	await assertWorkspaceMember(u.id, t.workspaceId);

	let body: { name?: unknown; color?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const patch: { name?: string; color?: string | null } = {};
	if ("name" in body) {
		const name = typeof body.name === "string" ? body.name.trim().slice(0, MAX_NAME) : "";
		if (!name) error(400, "Tag name can't be empty");
		patch.name = name;
	}
	if ("color" in body) patch.color = cleanColor(body.color);
	if (Object.keys(patch).length === 0) error(400, "Nothing to update");

	const db = getDb();
	try {
		await db.update(tag).set(patch).where(eq(tag.id, t.id));
	} catch (e) {
		if (((e as Error)?.message ?? String(e)).includes("tag_workspace_name_key")) {
			error(409, "A tag with that name already exists");
		}
		throw e;
	}
	return json({ ok: true, ...patch });
};

/** DELETE /api/tags/[id] — remove a tag (its recast assignments cascade). */
export const DELETE: RequestHandler = async ({ params, request }) => {
	const u = await requireUser(request);
	const t = await loadTag(params.id);
	if (!t) error(404, "Tag not found");
	await assertWorkspaceMember(u.id, t.workspaceId);

	const db = getDb();
	await db.delete(tag).where(eq(tag.id, t.id));
	return json({ ok: true });
};
