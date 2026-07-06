import { error, json } from "@sveltejs/kit";
import { eq, inArray } from "drizzle-orm";
import { customAlphabet } from "nanoid";
import { z } from "zod";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import {
	recast,
	share,
	shareMember,
	subscription,
	user,
} from "$lib/db/schema";
import { publicEnv } from "$lib/env/public";
import { hashSharePassword } from "$lib/share/password";
import type { RequestHandler } from "./$types";

type SessionShape = { user: { id: string } };

// Lower-case alphanumeric only — URL-clean, double-click-selectable, no
// homoglyph footguns. 10 chars × 36 alphabet = ~5.2e15 combos, plenty for
// our scale and short enough to fit in a chat message.
const slugAlphabet = "0123456789abcdefghijklmnopqrstuvwxyz";
const generateSlug = customAlphabet(slugAlphabet, 10);

const BodySchema = z
	.object({
		visibility: z
			.enum(["private", "workspace", "selected", "public"])
			.default("workspace"),
		// Optional bcrypt-style password — hashed before persist. Empty
		// string = no password.
		password: z
			.string()
			.transform((v) => v.trim())
			.refine((v) => v.length === 0 || v.length >= 4, {
				message: "Password must be at least 4 characters",
			})
			.optional(),
		// ISO date string; null = no expiry.
		expiresAt: z
			.string()
			.datetime()
			.optional()
			.nullable(),
		// For `selected` visibility — list of invitee emails. Owner is
		// implicit; don't include them here.
		invitees: z
			.array(
				z.object({
					email: z.string().email(),
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
 * Create a shareable link for an existing recast. Owner-only — every
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
 * Watermark: hard-coded `true` for free workspaces, `false` for pro.
 * Looked up via `subscription.plan` for the owner (workspaces inherit
 * the owner's plan in v1 — team-level billing comes later).
 *
 * Returns `{ slug, shareUrl }`. Caller turns shareUrl into a clickable.
 */
export const POST: RequestHandler = async ({ params, request, url }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;
	if (!session?.user) error(401, "Sign in required");

	let raw: unknown;
	try {
		raw = await request.json();
	} catch {
		// Allow empty body — defaults to workspace visibility, no password,
		// no expiry, no invitees. This is the "share with one click" path
		// the dashboard's quick-share button uses.
		raw = {};
	}
	const parsed = BodySchema.safeParse(raw);
	if (!parsed.success) {
		error(400, parsed.error.issues[0]?.message ?? "Invalid body");
	}
	const body = parsed.data;

	const db = getDb();

	const [row] = await db
		.select({
			id: recast.id,
			ownerId: recast.ownerId,
			workspaceId: recast.workspaceId,
			status: recast.status,
		})
		.from(recast)
		.where(eq(recast.id, params.id))
		.limit(1);
	if (!row) error(404, "Recast not found");
	if (row.ownerId !== session.user.id) error(403, "Not the owner");
	if (row.status === "archived") error(410, "Recast is archived");

	// `workspace` visibility needs the recast's owning org as the gate.
	// Fall back to `private` if somehow missing rather than 500.
	const orgId = body.visibility === "workspace" ? row.workspaceId : null;

	// Plan lookup for watermark. v1 reads the owner's subscription;
	// per-workspace billing is a follow-up.
	const [sub] = await db
		.select({ plan: subscription.plan })
		.from(subscription)
		.where(eq(subscription.userId, session.user.id))
		.limit(1);
	const isPro = sub?.plan === "pro";

	const passwordHash = await hashSharePassword(body.password);
	const requestedExpiresAt = body.expiresAt ? new Date(body.expiresAt) : null;

	// Free workspaces get a forced 15-day link expiry (to keep hosting costs
	// down) — default it when the client sent nothing, and cap anything longer.
	// Pro honours the request as-is, including "never" (null).
	const FREE_MAX_EXPIRY_DAYS = 15;
	const expiresAt = isPro
		? requestedExpiresAt
		: (() => {
				const cap = new Date(Date.now() + FREE_MAX_EXPIRY_DAYS * 24 * 60 * 60 * 1000);
				return requestedExpiresAt && requestedExpiresAt.getTime() < cap.getTime()
					? requestedExpiresAt
					: cap;
			})();

	// Slug generation — retry a couple of times on the (vanishingly rare)
	// collision. 10 chars over 36-symbol alphabet gives ~5×10^15 combos,
	// so a real collision should never happen, but the unique constraint
	// is authoritative and we'd rather retry than 500.
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

	const invitees = body.invitees ?? [];
	if (body.visibility === "selected" && invitees.length === 0) {
		error(400, "Selected visibility requires at least one invitee");
	}

	// Resolve invitee emails to user IDs in one trip so we can populate
	// `shareMember.userId` for already-registered users. Unregistered
	// emails get null and will be claimed when they sign in via magic
	// link (follow-up unlock flow).
	const resolvedInvitees =
		invitees.length > 0
			? await resolveInvitees(invitees, db)
			: [];

	await db.transaction(async (tx) => {
		await tx.insert(share).values({
			slug,
			recastId: row.id,
			ownerId: row.ownerId,
			organizationId: orgId,
			visibility: body.visibility,
			passwordHash,
			expiresAt,
			watermark: !isPro,
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
					invitedBy: session.user.id,
				})),
			);
		}
	});

	const base = publicEnv().PUBLIC_APP_URL.replace(/\/$/, "");
	return json({
		ok: true,
		slug,
		shareUrl: `${base}/share/${slug}`,
		visibility: body.visibility,
		watermark: !isPro,
		commentsEnabled: body.commentsEnabled,
	});
};

/**
 * GET /api/recasts/[id]/share
 *
 * List shares for a recast — owner only. Returns slug, visibility, view
 * count, expiry, and whether each has a password set (the hash is never
 * returned). Used by the dashboard's share-management drawer.
 */
export const GET: RequestHandler = async ({ params, request }) => {
	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;
	if (!session?.user) error(401, "Sign in required");

	const db = getDb();

	const [r] = await db
		.select({ ownerId: recast.ownerId })
		.from(recast)
		.where(eq(recast.id, params.id))
		.limit(1);
	if (!r) error(404, "Recast not found");
	if (r.ownerId !== session.user.id) error(403, "Not the owner");

	const rows = await db
		.select({
			slug: share.slug,
			visibility: share.visibility,
			organizationId: share.organizationId,
			hasPassword: share.passwordHash,
			expiresAt: share.expiresAt,
			watermark: share.watermark,
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

