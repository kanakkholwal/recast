import { error, json } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { member, recast, share, shareMember, user } from "$lib/db/schema";
import { grantCookieName, normalizeEmail, readGrantedEmail } from "$lib/share/grant";
import { constantTimeEquals, unlockCookieName, unlockToken } from "$lib/share/password";
import { isStorageConfigured, signDownloadUrl } from "$lib/storage";
import { deliveryState, getQuotaSnapshot, recordDelivery } from "$lib/storage/quota";
import type { RequestHandler } from "./$types";

type SessionShape = {
	user: { id: string; email: string; role?: string };
};

const SIGNED_URL_TTL_SECONDS = 60 * 60; // 1h

/**
 * GET /api/share/[id]/video
 *
 * Returns a short-lived signed GET URL for the share's underlying video,
 * after enforcing the visibility / password gates.
 *
 * Visibility:
 *   - `public`              → anyone, no auth needed
 *   - `workspace` / `team`  → signed-in member of the share's org
 *   - `selected`            → signed-in user whose email is in share_member
 *                             (or whose `user_id` matches that row)
 *   - `private`             → owner or global admin only
 *
 * Password (when `passwordHash` is set) is checked AFTER visibility — we
 * still want unauthorized viewers to get 401/403 instead of being prompted
 * for a password they can't possibly satisfy. The actual /unlock endpoint
 * setting the cookie is a follow-up; this handler refuses 401 with a
 * `reason: "password_required"` body so the player can render the prompt.
 */
export const GET: RequestHandler = async ({ params, request, cookies }) => {
	if (!isStorageConfigured()) error(503, "Cloud playback is not configured");

	const db = getDb();

	const [s] = await db
		.select({
			slug: share.slug,
			recastId: share.recastId,
			ownerId: share.ownerId,
			organizationId: share.organizationId,
			visibility: share.visibility,
			passwordHash: share.passwordHash,
			expiresAt: share.expiresAt,
		})
		.from(share)
		.where(eq(share.slug, params.id))
		.limit(1);
	if (!s) error(404, "Share not found");

	if (s.expiresAt && s.expiresAt.getTime() < Date.now()) {
		return json({ ok: false, reason: "expired" }, { status: 410 });
	}

	// Resolve session up front — most paths need it.
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	switch (s.visibility) {
		case "public":
			break;

		case "workspace":
		case "team": {
			if (!session?.user) error(401, "Sign in required");
			if (!s.organizationId) error(403, "Workspace share missing org");
			const [m] = await db
				.select({ id: member.id })
				.from(member)
				.where(and(eq(member.userId, session.user.id), eq(member.organizationId, s.organizationId)))
				.limit(1);
			if (!m) error(403, "Not a member of this workspace");
			break;
		}

		case "selected": {
			// Owner short-circuits.
			if (session?.user && s.ownerId === session.user.id) break;

			// Identity can be a signed-in email or an account-less grant cookie; both are re-checked against the allowlist below.
			const grantedEmail = await readGrantedEmail(s.slug, cookies.get(grantCookieName(s.slug)));
			const candidates = [session?.user?.email, grantedEmail]
				.filter((e): e is string => Boolean(e))
				.map(normalizeEmail);

			if (candidates.length === 0) {
				// No session and no grant: signal the player to render the request-access prompt rather than a bare 401.
				return json({ ok: false, reason: "claim_required" }, { status: 401 });
			}

			const members = await db
				.select({ email: shareMember.email })
				.from(shareMember)
				.where(eq(shareMember.shareSlug, s.slug));
			const allow = new Set(members.map((m) => normalizeEmail(m.email)));
			if (!candidates.some((e) => allow.has(e))) {
				error(403, "Not on the access list");
			}
			break;
		}

		case "private": {
			if (!session?.user) error(401, "Sign in required");
			if (session.user.id === s.ownerId) break;
			// Re-read the role server-side rather than trusting the session, so a role change takes effect next request.
			const [u] = await db
				.select({ role: user.role })
				.from(user)
				.where(eq(user.id, session.user.id))
				.limit(1);
			if (u?.role !== "admin") error(403, "Private share");
			break;
		}

		default:
			error(500, "Unknown visibility");
	}

	if (s.passwordHash) {
		const got = cookies.get(unlockCookieName(s.slug));
		const expected = await unlockToken(s.slug);
		if (!got || !constantTimeEquals(got, expected)) {
			return json({ ok: false, reason: "password_required" }, { status: 401 });
		}
	}

	const [r] = await db
		.select({
			videoUrl: recast.videoUrl,
			status: recast.status,
			sizeBytes: recast.sizeBytes,
		})
		.from(recast)
		.where(eq(recast.id, s.recastId))
		.limit(1);
	if (!r) error(404, "Underlying recast missing");
	if (r.status === "archived") {
		return json({ ok: false, reason: "archived" }, { status: 410 });
	}

	// Egress dominates infra cost, so the allowance is enforced where a playable URL is handed out; paid plans stream past the cap as overage.
	if (s.organizationId) {
		const snapshot = await getQuotaSnapshot(s.organizationId);
		if (snapshot) {
			const delivery = deliveryState(snapshot);
			if (delivery.exceeded && snapshot.plan === "free") {
				return json(
					{
						ok: false,
						reason: "delivery_cap_reached",
						capBytes: delivery.capBytes,
					},
					{ status: 402 },
				);
			}
		}
	}

	const url = await signDownloadUrl({
		key: r.videoUrl,
		expiresInSeconds: SIGNED_URL_TTL_SECONDS,
	});

	// Counted per URL grant at full file size, an upper bound kept deliberately conservative because it guards the bill.
	if (s.organizationId && r.sizeBytes) {
		await recordDelivery(s.organizationId, r.sizeBytes).catch((err) => {
			console.error("[delivery] failed to record", err);
		});
	}

	return json({
		ok: true,
		url,
		expiresInSeconds: SIGNED_URL_TTL_SECONDS,
	});
};
