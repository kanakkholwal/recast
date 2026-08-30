import { redirect } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { dev } from "$app/environment";
import { getDb } from "$lib/db";
import { share, shareMember } from "$lib/db/schema";
import {
	constantTimeEquals,
	grantCookieName,
	grantCookieValue,
	grantToken,
	normalizeEmail,
} from "$lib/share/grant";
import type { RequestHandler } from "./$types";

const GRANT_MAX_AGE = 60 * 60 * 24 * 30; // 30 days

/**
 * GET /api/share/[id]/claim/verify?e=<email>&t=<token>
 *
 * The target of the emailed access link. Recomputes the grant token; on a
 * match (and a still-valid allowlist membership) it sets the httpOnly grant
 * cookie and bounces the viewer to the share page, now unlocked. Any failure
 * lands on the share page with `?claim=invalid` so the UI can re-prompt
 * rather than dead-end.
 */
export const GET: RequestHandler = async ({ params, url, cookies }) => {
	const slug = params.id;
	const sharePath = `/share/${encodeURIComponent(slug)}`;

	const email = normalizeEmail(url.searchParams.get("e") ?? "");
	const token = url.searchParams.get("t") ?? "";
	if (!email || !token) redirect(303, `${sharePath}?claim=invalid`);

	const expected = await grantToken(slug, email);
	// Constant-time compare: this validates an HMAC-derived capability token, so a prefix timing leak matters.
	if (!constantTimeEquals(token, expected)) {
		redirect(303, `${sharePath}?claim=invalid`);
	}

	// Re-check the allowlist and that the share is still invite-only, so removing an invitee revokes the link.
	const db = getDb();
	const [s] = await db
		.select({ visibility: share.visibility })
		.from(share)
		.where(eq(share.slug, slug))
		.limit(1);
	if (s?.visibility !== "selected") {
		redirect(303, `${sharePath}?claim=invalid`);
	}
	const [allowed] = await db
		.select({ id: shareMember.id })
		.from(shareMember)
		.where(and(eq(shareMember.shareSlug, slug), eq(shareMember.email, email)))
		.limit(1);
	if (!allowed) redirect(303, `${sharePath}?claim=invalid`);

	cookies.set(grantCookieName(slug), await grantCookieValue(slug, email), {
		path: "/",
		httpOnly: true,
		secure: !dev,
		sameSite: "lax",
		maxAge: GRANT_MAX_AGE,
	});

	redirect(303, sharePath);
};
