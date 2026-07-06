import { error, json } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { shareReaction } from "$lib/db/schema";
import { gateShareAccess } from "$lib/share/gate";
import { reactorKey, resolveClientIp } from "$lib/share/ip";
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
 * Set a reaction. Always allowed (independent of the comments toggle) —
 * reactions are the lighter, lower-abuse engagement surface. A viewer gets ONE
 * reaction per share, keyed by their IP (`reactorKey`), so one IP maps to one
 * reaction:
 *   - no current reaction → add it
 *   - same emoji tapped again → remove it (toggle off)
 *   - a different emoji → switch in place (never stacks)
 * `atSeconds` is recorded as owner-insight metadata, not part of identity.
 *
 * Body: { sessionId, emoji, atSeconds }
 */
export const POST: RequestHandler = async ({
	params,
	request,
	cookies,
	getClientAddress,
}) => {
	const gate = await gateShareAccess(params.id, request, cookies);

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

	const key = reactorKey({
		userId: gate.viewerId,
		ip: resolveClientIp(request, getClientAddress),
		sessionId,
	});
	const db = getDb();

	// The reactor's current reaction, if any — one row per (share, reactor).
	const [existing] = await db
		.select({ id: shareReaction.id, emoji: shareReaction.emoji })
		.from(shareReaction)
		.where(
			and(eq(shareReaction.shareSlug, params.id), eq(shareReaction.ipHash, key)),
		)
		.limit(1);

	if (existing) {
		if (existing.emoji === emoji) {
			await db.delete(shareReaction).where(eq(shareReaction.id, existing.id));
			return json({ ok: true, added: false });
		}
		// Switch reaction type in place — keeps it a single reaction per viewer.
		await db
			.update(shareReaction)
			.set({ emoji, atSeconds, createdAt: new Date() })
			.where(eq(shareReaction.id, existing.id));
		return json({ ok: true, added: true, emoji });
	}

	await db.insert(shareReaction).values({
		id: crypto.randomUUID(),
		shareSlug: params.id,
		sessionId,
		ipHash: key,
		emoji,
		atSeconds,
	});
	return json({ ok: true, added: true, emoji }, { status: 201 });
};
