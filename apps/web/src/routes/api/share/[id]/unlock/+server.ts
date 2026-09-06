import { error, json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { z } from "zod";
import { dev } from "$app/environment";
import { getDb } from "$lib/db";
import { share } from "$lib/db/schema";
import { enforceRateLimit } from "$lib/server/rate-limit";
import { unlockCookieName, unlockToken, verifySharePassword } from "$lib/share/password";
import type { RequestHandler } from "./$types";

const BodySchema = z.object({
	password: z.string().min(1).max(200),
});

// A 7-day cookie outlives a viewing session but rotates often; with no issued-at column, rotating BETTER_AUTH_SECRET is the only other lever.
const COOKIE_MAX_AGE_SECONDS = 7 * 24 * 60 * 60;

/**
 * POST /api/share/[id]/unlock
 *
 * Verifies the supplied password against `share.passwordHash`, sets a
 * per-share unlock cookie on success, and returns `{ ok: true }`. The
 * cookie is HttpOnly so it can't be read by JS — only the /video
 * endpoint reads it. SameSite=Lax so it works on follow-up navigations
 * to the share page without being usable for CSRF on third-party sites.
 *
 * Body: `{ password: string }`.
 * Returns: `{ ok: true }` (200) on success, `{ ok: false }` (401) on
 * mismatch, 404 if the share doesn't exist, 400 if no password is set
 * (caller shouldn't be calling unlock then).
 */
export const POST: RequestHandler = async ({ params, request, cookies, getClientAddress }) => {
	// Throttle per share and IP before touching the DB or hashing, so this can't brute-force `share.passwordHash`.
	const limited = await enforceRateLimit(
		{ getClientAddress },
		{ bucket: "share-unlock", id: params.id, limit: 10, windowMs: 60_000 },
	);
	if (limited) return limited;

	let raw: unknown;
	try {
		raw = await request.json();
	} catch {
		error(400, "Invalid JSON body");
	}
	const parsed = BodySchema.safeParse(raw);
	if (!parsed.success) {
		error(422, parsed.error.issues[0]?.message ?? "Invalid body");
	}

	const db = getDb();
	const [row] = await db
		.select({
			slug: share.slug,
			passwordHash: share.passwordHash,
		})
		.from(share)
		.where(eq(share.slug, params.id))
		.limit(1);
	if (!row) error(404, "Share not found");
	if (!row.passwordHash) error(400, "Share has no password");

	const ok = await verifySharePassword(parsed.data.password, row.passwordHash);
	if (!ok) {
		return json({ ok: false, reason: "invalid_password" }, { status: 401 });
	}

	const token = await unlockToken(row.slug);
	cookies.set(unlockCookieName(row.slug), token, {
		path: "/",
		httpOnly: true,
		sameSite: "lax",
		secure: !dev,
		maxAge: COOKIE_MAX_AGE_SECONDS,
	});

	return json({ ok: true });
};
