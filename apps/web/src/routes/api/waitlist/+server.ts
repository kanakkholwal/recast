import { json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { z } from "zod";
import { getDb } from "$lib/db";
import { user, waitlist } from "$lib/db/schema";
import { enforceRateLimit } from "$lib/server/rate-limit";
import type { RequestHandler } from "./$types";

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

// Schema is the single source of truth for the request body. `source`/`name`
// stay lenient (coerce non-strings to a safe default rather than rejecting) to
// preserve the prior hand-rolled behaviour; only `email` gates the request.
const BodySchema = z.object({
	email: z
		.string()
		.transform((v) => v.trim().toLowerCase())
		.refine((v) => EMAIL_RE.test(v), "Invalid email"),
	// `.optional()` is load-bearing: a bare `z.unknown()` accepts undefined, but
	// wrapping it in `.transform()` makes the key required, so a body without
	// `name` (what the client actually sends) fails the whole parse.
	source: z
		.unknown()
		.optional()
		.transform((v) => (typeof v === "string" ? v.slice(0, 64) : null)),
	name: z
		.unknown()
		.optional()
		.transform((v) => (typeof v === "string" ? v.trim().slice(0, 80) : "")),
});

/**
 * Waitlist sign-up. Idempotent: hitting it twice with the same email is a
 * no-op. Creates two rows when a new email comes in:
 *
 *   1. A `user` row with `status = "pending"` and no credential account, so
 *      the email is reserved and the user shows up in admin tooling. They
 *      can't sign in until an admin flips status → "active" (inline from
 *      /admin/users/[id] or in bulk from /admin/waitlist). Magic-link and
 *      password-reset hooks both gate on this column.
 *
 *   2. A `waitlist` row capturing the funnel source.
 *
 * No password is set; once activated, the user picks one via password reset
 * or just signs in with a magic link. `role` stays at the plugin default
 * ("user") — admin promotion is a separate, deliberate flip.
 */
export const POST: RequestHandler = async ({ request, getClientAddress }) => {
	// Unauthenticated and creates a `user` row — throttle hard per IP so it
	// can't be used to bulk-insert junk accounts.
	const limited = await enforceRateLimit(
		{ getClientAddress },
		{ bucket: "waitlist", limit: 5, windowMs: 10 * 60_000 },
	);
	if (limited) return limited;

	let raw: unknown;
	try {
		raw = await request.json();
	} catch {
		return json({ ok: false, error: "Invalid JSON" }, { status: 400 });
	}

	const parsed = BodySchema.safeParse(raw);
	if (!parsed.success) {
		// Don't blame the email for a failure somewhere else in the body.
		const onEmail = parsed.error.issues.some((i) => i.path[0] === "email");
		return json(
			{ ok: false, error: onEmail ? "Invalid email" : "Invalid request." },
			{ status: 422 },
		);
	}
	const { email, source } = parsed.data;
	const requestedName = parsed.data.name;

	const db = getDb();

	// 1. Create the user row (idempotent). If a user with this email already
	//    exists — active or pending — we leave it alone.
	const existing = await db
		.select({ id: user.id, status: user.status })
		.from(user)
		.where(eq(user.email, email))
		.limit(1);

	if (existing.length === 0) {
		await db.insert(user).values({
			id: crypto.randomUUID(),
			email,
			name: requestedName || email.split("@")[0]!,
			status: "pending",
		});
	}

	// 2. Capture the waitlist event (idempotent on email).
	await db
		.insert(waitlist)
		.values({ id: crypto.randomUUID(), email, source })
		.onConflictDoNothing({ target: waitlist.email });

	return json({ ok: true });
};
