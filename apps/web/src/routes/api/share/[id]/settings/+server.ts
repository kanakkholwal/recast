import { error, json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { recast, share } from "$lib/db/schema";
import { resolveShareManage } from "$lib/share/manage";
import { hashSharePassword } from "$lib/share/password";
import type { RequestHandler } from "./$types";

type SessionShape = { user: { id: string; role?: string } };

const MAX_CTA_LABEL = 60;
const MAX_DESCRIPTION = 500;
const MIN_PASSWORD = 4;

/**
 * PATCH /api/share/[id]/settings
 *
 * Owner-or-admin endpoint for the non-visibility share knobs surfaced in the
 * share menu / desktop manage drawer. Visibility lives in the sibling
 * `/access` endpoint.
 *
 * Body (all optional, only provided keys are written):
 *   - ctaLabel, ctaUrl : both-or-neither. Empty/null on either clears the
 *                        CTA. ctaUrl must be an absolute http(s) URL.
 *   - commentsEnabled  : boolean
 *   - password         : string (≥4 chars) to set, "" or null to remove.
 *                        Hashed before persist; never echoed back.
 *   - expiresAt        : ISO datetime to set, null to clear. Must be future.
 */
export const PATCH: RequestHandler = async ({ params, request }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;
	if (!session?.user) error(401, "Sign in required");

	let body: {
		ctaLabel?: unknown;
		ctaUrl?: unknown;
		commentsEnabled?: unknown;
		password?: unknown;
		expiresAt?: unknown;
		description?: unknown;
	} = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		error(400, "Invalid JSON body");
	}

	const db = getDb();
	// Authorize: share owner, an owner/admin of the recast's workspace, or a
	// global admin. Roles are re-read from the DB so changes take effect at once.
	const manage = await resolveShareManage(params.id, session.user.id);
	if (!manage) error(404, "Share not found");
	if (!manage.canManage) error(403, "Not allowed to change this share");

	const patch: {
		ctaLabel?: string | null;
		ctaUrl?: string | null;
		commentsEnabled?: boolean;
		passwordHash?: string | null;
		expiresAt?: Date | null;
	} = {};

	// CTA is both-or-neither: a label without a destination (or vice versa)
	// renders a dead button, so we treat a partial input as "clear".
	const hasCta = "ctaLabel" in body || "ctaUrl" in body;
	if (hasCta) {
		const label =
			typeof body.ctaLabel === "string" ? body.ctaLabel.trim().slice(0, MAX_CTA_LABEL) : "";
		const rawUrl = typeof body.ctaUrl === "string" ? body.ctaUrl.trim() : "";
		if (label && rawUrl) {
			let parsed: URL;
			try {
				parsed = new URL(rawUrl);
			} catch {
				error(400, "CTA link must be a valid URL");
			}
			if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
				error(400, "CTA link must be an http(s) URL");
			}
			patch.ctaLabel = label;
			patch.ctaUrl = parsed.toString();
		} else {
			patch.ctaLabel = null;
			patch.ctaUrl = null;
		}
	}

	if (typeof body.commentsEnabled === "boolean") {
		patch.commentsEnabled = body.commentsEnabled;
	}

	// Password: a non-empty string sets (and hashes) it; "" or null clears.
	if ("password" in body) {
		if (body.password === null || body.password === "") {
			patch.passwordHash = null;
		} else if (typeof body.password === "string") {
			if (body.password.length < MIN_PASSWORD) {
				error(400, `Password must be at least ${MIN_PASSWORD} characters`);
			}
			patch.passwordHash = await hashSharePassword(body.password);
		} else {
			error(400, "Invalid password");
		}
	}

	// Expiry: an ISO datetime sets it (must be future); null clears.
	if ("expiresAt" in body) {
		if (body.expiresAt === null) {
			patch.expiresAt = null;
		} else if (typeof body.expiresAt === "string") {
			const when = new Date(body.expiresAt);
			if (Number.isNaN(when.getTime())) error(400, "Invalid expiry date");
			if (when.getTime() <= Date.now()) error(400, "Expiry must be in the future");
			patch.expiresAt = when;
		} else {
			error(400, "Invalid expiry date");
		}
	}

	// Description lives on the recast (it's the video's blurb, reused for the
	// OG card), not the share row — updated separately below.
	let description: string | null | undefined;
	if ("description" in body) {
		description =
			typeof body.description === "string" && body.description.trim()
				? body.description.trim().slice(0, MAX_DESCRIPTION)
				: null;
	}

	if (Object.keys(patch).length === 0 && description === undefined) {
		error(400, "Nothing to update");
	}

	if (Object.keys(patch).length > 0) {
		await db.update(share).set(patch).where(eq(share.slug, params.id));
	}
	if (description !== undefined) {
		await db.update(recast).set({ description }).where(eq(recast.id, manage.recastId));
	}

	// Never echo the password hash back. Report whether a password is now set
	// and the other (safe) fields that changed.
	const { passwordHash, expiresAt, ...safe } = patch;
	return json({
		ok: true,
		...safe,
		...(description !== undefined ? { description } : {}),
		...("passwordHash" in patch ? { passwordSet: passwordHash !== null } : {}),
		...("expiresAt" in patch
			? { expiresAt: expiresAt ? expiresAt.toISOString() : null }
			: {}),
	});
};
