import { error, json } from "@sveltejs/kit";
import { and, eq, inArray } from "drizzle-orm";
import { customAlphabet } from "nanoid";
import { z } from "zod";
import { getDb } from "$lib/db";
import { organization, share, shareMember, user } from "$lib/db/schema";
import { publicEnv } from "$lib/env/public";
import { authorizeRecast } from "$lib/server/recast-guard";
import { hashSharePassword, verifySharePassword } from "$lib/share/password";
import { emailField } from "$lib/validation/email";
import type { RequestHandler } from "./$types";

// Lower-case alphanumeric only: URL-clean, double-click-selectable, no homoglyphs, and 36^10 is ample.
const slugAlphabet = "0123456789abcdefghijklmnopqrstuvwxyz";
const generateSlug = customAlphabet(slugAlphabet, 10);

const BodySchema = z
	.object({
		visibility: z.enum(["private", "workspace", "selected", "public"]).default("workspace"),
		// Optional password, hashed before persist; an empty string means none.
		password: z
			.string()
			.transform((v) => v.trim())
			.refine((v) => v.length === 0 || v.length >= 4, {
				message: "Password must be at least 4 characters",
			})
			.optional(),
		// ISO date string; null = no expiry.
		expiresAt: z.iso.datetime().nullish(),
		// Invitee emails for `selected` visibility; the owner is implicit and must not be listed.
		invitees: z
			.array(
				z.object({
					email: emailField(),
					role: z.enum(["viewer", "commenter"]).default("viewer"),
				}),
			)
			.max(50)
			.optional(),
		commentsEnabled: z.boolean().default(true),
	})
	.strict();

/**
 * POST /api/recasts/[id]/share
 *
 * Create a shareable link for an existing recast. Creator, workspace
 * owner/admin, or platform admin. Every
 * recast can have any number of shares (different visibilities, different
 * passwords, different selected lists), but all are owned by the original
 * recast owner.
 *
 * Selected-visibility flow:
 *   - Caller supplies `invitees: [{ email, role }]`.
 *   - We resolve each email against `user.email` if it exists, otherwise
 *     leave `userId` null — the magic-link-on-first-view flow (later)
 *     can fill it in.
 *   - The owner is implicitly allowed; don't include them.
 *
 * Looked up via `subscription.plan` for the owner (workspaces inherit
 * the owner's plan in v1 — team-level billing comes later).
 *
 * Returns `{ slug, shareUrl }`. Caller turns shareUrl into a clickable.
 */
export const POST: RequestHandler = async ({ params, request }) => {
	let raw: unknown;
	try {
		raw = await request.json();
	} catch {
		// An empty body is the dashboard's one-click share: workspace visibility, no password, expiry or invitees.
		raw = {};
	}
	const parsed = BodySchema.safeParse(raw);
	if (!parsed.success) {
		error(400, parsed.error.issues[0]?.message ?? "Invalid body");
	}
	const body = parsed.data;

	const db = getDb();

	const row = await authorizeRecast(request, params.id);
	if (row.status === "archived") error(410, "Recast is archived");

	// `workspace` visibility gates on the owning org; fall back to `private` rather than 500.
	const orgId = body.visibility === "workspace" ? row.workspaceId : null;

	// Read the plan from the quota snapshot's source, and treat both paid tiers as Pro: `=== 'pro'` demoted Enterprise.
	const [org] = await db
		.select({ plan: organization.plan })
		.from(organization)
		.where(eq(organization.id, row.workspaceId))
		.limit(1);
	const isPro = org?.plan === "pro" || org?.plan === "enterprise";

	const passwordHash = await hashSharePassword(body.password);
	const requestedExpiresAt = body.expiresAt ? new Date(body.expiresAt) : null;

	// Free workspaces get a forced 15-day expiry; Pro honours the request as-is, including never.
	const FREE_MAX_EXPIRY_DAYS = 15;
	const expiresAt = isPro
		? requestedExpiresAt
		: (() => {
				const cap = new Date(Date.now() + FREE_MAX_EXPIRY_DAYS * 24 * 60 * 60 * 1000);
				return requestedExpiresAt && requestedExpiresAt.getTime() < cap.getTime()
					? requestedExpiresAt
					: cap;
			})();

	const invitees = body.invitees ?? [];
	if (body.visibility === "selected" && invitees.length === 0) {
		error(400, "Selected visibility requires at least one invitee");
	}

	const base = publicEnv().PUBLIC_APP_URL.replace(/\/$/, "");

	// Dedup an identical live link (same visibility, comments, expiry, password and invitees); an expiring link rarely matches, which is intended.
	const reqExpiry = expiresAt ? expiresAt.getTime() : null;
	const reqEmails = new Set(invitees.map((i) => i.email.toLowerCase()));
	const candidates = await db
		.select({
			slug: share.slug,
			passwordHash: share.passwordHash,
			expiresAt: share.expiresAt,
			commentsEnabled: share.commentsEnabled,
		})
		.from(share)
		.where(and(eq(share.recastId, row.id), eq(share.visibility, body.visibility)));
	for (const c of candidates) {
		if (Boolean(c.commentsEnabled) !== Boolean(body.commentsEnabled)) continue;
		if ((c.expiresAt ? c.expiresAt.getTime() : null) !== reqExpiry) continue;
		if (Boolean(c.passwordHash) !== Boolean(body.password)) continue;
		if (body.password && !(await verifySharePassword(body.password, c.passwordHash))) continue;
		if (body.visibility === "selected") {
			const members = await db
				.select({ email: shareMember.email })
				.from(shareMember)
				.where(eq(shareMember.shareSlug, c.slug));
			const have = new Set(members.map((m) => m.email.toLowerCase()));
			if (have.size !== reqEmails.size || [...reqEmails].some((e) => !have.has(e))) continue;
		}
		return json({
			ok: true,
			slug: c.slug,
			shareUrl: `${base}/share/${c.slug}`,
			visibility: body.visibility,
			commentsEnabled: body.commentsEnabled,
			deduped: true,
		});
	}

	// Retry a couple of times on the vanishingly rare collision: the unique constraint is authoritative and beats a 500.
	let slug = generateSlug();
	let attempts = 0;
	while (attempts < 3) {
		const [existing] = await db
			.select({ slug: share.slug })
			.from(share)
			.where(eq(share.slug, slug))
			.limit(1);
		if (!existing) break;
		slug = generateSlug();
		attempts++;
	}

	// Resolve invitees in one trip; unregistered emails get null and are claimed on magic-link sign-in.
	const resolvedInvitees = invitees.length > 0 ? await resolveInvitees(invitees, db) : [];

	await db.transaction(async (tx) => {
		await tx.insert(share).values({
			slug,
			recastId: row.id,
			ownerId: row.ownerId,
			organizationId: orgId,
			visibility: body.visibility,
			passwordHash,
			expiresAt,
			commentsEnabled: body.commentsEnabled,
		});

		if (resolvedInvitees.length > 0) {
			await tx.insert(shareMember).values(
				resolvedInvitees.map((inv) => ({
					id: crypto.randomUUID(),
					shareSlug: slug,
					email: inv.email,
					userId: inv.userId,
					role: inv.role,
					invitedBy: row.userId,
				})),
			);
		}
	});

	return json({
		ok: true,
		slug,
		shareUrl: `${base}/share/${slug}`,
		visibility: body.visibility,
		commentsEnabled: body.commentsEnabled,
	});
};

/**
 * GET /api/recasts/[id]/share
 *
 * List shares for a recast. Returns slug, visibility, view
 * count, expiry, and whether each has a password set (the hash is never
 * returned). Used by the dashboard's share-management drawer.
 */
export const GET: RequestHandler = async ({ params, request }) => {
	const db = getDb();

	await authorizeRecast(request, params.id);

	const rows = await db
		.select({
			slug: share.slug,
			visibility: share.visibility,
			organizationId: share.organizationId,
			hasPassword: share.passwordHash,
			expiresAt: share.expiresAt,
			viewsCount: share.viewsCount,
			createdAt: share.createdAt,
		})
		.from(share)
		.where(eq(share.recastId, params.id));

	return json({
		ok: true,
		shares: rows.map((r) => ({
			...r,
			hasPassword: Boolean(r.hasPassword),
		})),
	});
};

/** Look up which invitee emails already have a user row. */
async function resolveInvitees(
	invitees: Array<{ email: string; role: "viewer" | "commenter" }>,
	db: ReturnType<typeof getDb>,
) {
	const emails = [...new Set(invitees.map((i) => i.email.toLowerCase()))];
	if (emails.length === 0) return [];
	const rows = await db
		.select({ id: user.id, email: user.email })
		.from(user)
		.where(inArray(user.email, emails));
	const byEmail = new Map(rows.map((r) => [r.email.toLowerCase(), r.id]));
	return invitees.map((inv) => ({
		email: inv.email,
		role: inv.role,
		userId: byEmail.get(inv.email.toLowerCase()) ?? null,
	}));
}
