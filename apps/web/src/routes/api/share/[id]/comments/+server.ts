import { error, json } from "@sveltejs/kit";
import { and, asc, eq, isNull } from "drizzle-orm";
import { getDb } from "$lib/db";
import { shareComment, shareReaction } from "$lib/db/schema";
import { enforceRateLimit } from "$lib/server/rate-limit";
import { gateShareAccess } from "$lib/share/gate";
import { reactorKey, resolveClientIp } from "$lib/share/ip";
import type { RequestHandler } from "./$types";

const MAX_NAME = 60;
const MAX_BODY = 2000;

/**
 * GET /api/share/[id]/comments
 *
 * Public (subject to the share's visibility/password gate). Returns the
 * comment thread plus aggregated reactions for the share. Pass `?sessionId=`
 * — the anonymous browser fingerprint — so the response can flag which
 * comments/reactions belong to the caller (drives self-delete + toggle UI)
 * without ever leaking other viewers' fingerprints.
 */
export const GET: RequestHandler = async ({
	params,
	request,
	cookies,
	url,
	getClientAddress,
}) => {
	const gate = await gateShareAccess(params.id, request, cookies);
	const sessionId = url.searchParams.get("sessionId") ?? "";
	// The caller's reaction identity — must match the POST handler so their own
	// reaction renders pressed. Account id when signed in, else IP, else session.
	const key = reactorKey({
		userId: gate.viewerId,
		ip: resolveClientIp(request, getClientAddress),
		sessionId,
	});

	const db = getDb();

	const rows = await db
		.select({
			id: shareComment.id,
			sessionId: shareComment.sessionId,
			authorName: shareComment.authorName,
			atSeconds: shareComment.atSeconds,
			body: shareComment.body,
			createdAt: shareComment.createdAt,
		})
		.from(shareComment)
		.where(
			and(eq(shareComment.shareSlug, params.id), isNull(shareComment.deletedAt)),
		)
		.orderBy(asc(shareComment.createdAt));

	const reactionRows = await db
		.select({
			emoji: shareReaction.emoji,
			ipHash: shareReaction.ipHash,
		})
		.from(shareReaction)
		.where(eq(shareReaction.shareSlug, params.id));

	// Aggregate reactions per emoji → count, and flag the caller's own (a single
	// reaction now) so the client can render the pressed state.
	const counts = new Map<string, number>();
	const mine: string[] = [];
	for (const r of reactionRows) {
		counts.set(r.emoji, (counts.get(r.emoji) ?? 0) + 1);
		if (r.ipHash && r.ipHash === key) mine.push(r.emoji);
	}

	return json({
		ok: true,
		commentsEnabled: gate.commentsEnabled,
		canManage: gate.canManage,
		comments: rows.map((c) => ({
			id: c.id,
			authorName: c.authorName,
			atSeconds: c.atSeconds,
			body: c.body,
			createdAt: c.createdAt.getTime(),
			mine: Boolean(sessionId) && c.sessionId === sessionId,
		})),
		reactions: [...counts.entries()].map(([emoji, count]) => ({ emoji, count })),
		myReactions: mine,
	});
};

/**
 * POST /api/share/[id]/comments
 *
 * Create a comment. Name-only identity — no account required; the viewer
 * supplies a display name and their anonymous `sessionId`. Refused when the
 * owner has disabled comments on this share (reactions stay open via the
 * sibling endpoint).
 *
 * Body: { sessionId, authorName, atSeconds, body }
 */
export const POST: RequestHandler = async ({ params, request, cookies, getClientAddress }) => {
	const limited = await enforceRateLimit(
		{ getClientAddress },
		{ bucket: "share-comment", id: params.id, limit: 15, windowMs: 60_000 },
	);
	if (limited) return limited;

	const gate = await gateShareAccess(params.id, request, cookies);
	if (!gate.commentsEnabled) error(403, "Comments are turned off for this share");

	let body: {
		sessionId?: unknown;
		authorName?: unknown;
		atSeconds?: unknown;
		body?: unknown;
	} = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const sessionId = typeof body.sessionId === "string" ? body.sessionId.trim() : "";
	const authorName =
		typeof body.authorName === "string" ? body.authorName.trim().slice(0, MAX_NAME) : "";
	const text = typeof body.body === "string" ? body.body.trim().slice(0, MAX_BODY) : "";
	const atSeconds =
		typeof body.atSeconds === "number" && Number.isFinite(body.atSeconds)
			? Math.max(0, Math.floor(body.atSeconds))
			: 0;

	if (!sessionId) error(400, "Missing session");
	if (!authorName) error(400, "A name is required");
	if (!text) error(400, "Comment can't be empty");

	const db = getDb();
	const id = crypto.randomUUID();
	const createdAt = new Date();

	await db.insert(shareComment).values({
		id,
		shareSlug: params.id,
		sessionId,
		authorName,
		atSeconds,
		body: text,
		createdAt,
	});

	return json(
		{
			ok: true,
			comment: {
				id,
				authorName,
				atSeconds,
				body: text,
				createdAt: createdAt.getTime(),
				mine: true,
			},
		},
		{ status: 201 },
	);
};
