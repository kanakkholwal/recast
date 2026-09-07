import { error, json } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { tag } from "$lib/db/schema";
import { assertWorkspaceMember, requireUser } from "$lib/workspace/guard";
import type { RequestHandler } from "./$types";

const MAX_NAME = 40;
const HEX = /^#[0-9a-fA-F]{6}$/;

function cleanColor(v: unknown): string | null {
	return typeof v === "string" && HEX.test(v.trim()) ? v.trim() : null;
}

/** GET /api/tags?workspaceId=… — all tags in a workspace. */
export const GET: RequestHandler = async ({ request, url }) => {
	const u = await requireUser(request);
	const workspaceId = url.searchParams.get("workspaceId");
	if (!workspaceId) error(400, "workspaceId is required");
	await assertWorkspaceMember(u.id, workspaceId);

	const db = getDb();
	const rows = await db
		.select({ id: tag.id, name: tag.name, color: tag.color })
		.from(tag)
		.where(eq(tag.workspaceId, workspaceId));

	return json({ ok: true, tags: rows });
};

/**
 * POST /api/tags — create a tag. Body: { workspaceId, name, color? }
 * Idempotent on (workspace, name): re-creating an existing tag returns it
 * (200) rather than erroring, so the UI's "type to create" stays friendly.
 */
export const POST: RequestHandler = async ({ request }) => {
	const u = await requireUser(request);

	let body: { workspaceId?: unknown; name?: unknown; color?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const workspaceId = typeof body.workspaceId === "string" ? body.workspaceId : "";
	const name = typeof body.name === "string" ? body.name.trim().slice(0, MAX_NAME) : "";
	if (!workspaceId) error(400, "workspaceId is required");
	if (!name) error(400, "Tag name is required");

	await assertWorkspaceMember(u.id, workspaceId);

	const db = getDb();
	const id = crypto.randomUUID();
	const color = cleanColor(body.color);

	const inserted = await db
		.insert(tag)
		.values({ id, workspaceId, name, color })
		.onConflictDoNothing({ target: [tag.workspaceId, tag.name] })
		.returning({ id: tag.id, name: tag.name, color: tag.color });

	if (inserted[0]) {
		return json({ ok: true, tag: inserted[0] }, { status: 201 });
	}

	// Already existed — return the existing row.
	const [existing] = await db
		.select({ id: tag.id, name: tag.name, color: tag.color })
		.from(tag)
		.where(and(eq(tag.workspaceId, workspaceId), eq(tag.name, name)))
		.limit(1);
	// The conflict fired but the row is gone, so fail loudly rather than ship a 200 with an undefined tag.
	if (!existing) error(500, "Tag creation conflicted but the existing tag could not be found");
	return json({ ok: true, tag: existing });
};
