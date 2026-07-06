import { error, json } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { shareReaction } from "$lib/db/schema";
import { gateShareAccess } from "$lib/share/gate";
import { REACTION_IDS } from "$lib/share/reactions";
import type { RequestHandler } from "./$types";

/**
 * Allowed reaction set — stable ids from the shared registry (the rendered
 * icon is mapped from the id on the client, so the id is what we persist). A
 * small curated palette keeps the surface tight and bounds abuse. Anchored to
 * a point in the video.
 */
const ALLOWED = new Set(REACTION_IDS);

/**
 * POST /api/share/[id]/reactions
 *
 * Toggle a reaction. Always allowed (independent of the comments toggle) —
 * reactions are the lighter, lower-abuse engagement surface. Name-only:
 * identity is the anonymous `sessionId`. One row per (share, viewer, emoji),
 * so re-posting the same emoji removes it — the button reads as a toggle.
 * `atSeconds` is recorded as owner-insight metadata, not part of identity.
 *
 * Body: { sessionId, emoji, atSeconds }
 */
export const POST: RequestHandler = async ({ params, request, cookies }) => {
	await gateShareAccess(params.id, request, cookies);

	let body: { sessionId?: unknown; emoji?: unknown; atSeconds?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const sessionId = typeof body.sessionId === "string" ? body.sessionId.trim() : "";
	const emoji = typeof body.emoji === "string" ? body.emoji : "";
	const atSeconds =
		typeof body.atSeconds === "number" && Number.isFinite(body.atSeconds)
			? Math.max(0, Math.floor(body.atSeconds))
			: 0;

	if (!sessionId) error(400, "Missing session");
	if (!ALLOWED.has(emoji)) error(400, "Unsupported reaction");

	const db = getDb();
	const match = and(
		eq(shareReaction.shareSlug, params.id),
		eq(shareReaction.sessionId, sessionId),
		eq(shareReaction.emoji, emoji),
	);

	const [existing] = await db
		.select({ id: shareReaction.id })
		.from(shareReaction)
		.where(match)
		.limit(1);

	if (existing) {
		await db.delete(shareReaction).where(eq(shareReaction.id, existing.id));
		return json({ ok: true, added: false });
	}

	await db.insert(shareReaction).values({
		id: crypto.randomUUID(),
		shareSlug: params.id,
		sessionId,
		emoji,
		atSeconds,
	});
	return json({ ok: true, added: true }, { status: 201 });
};
