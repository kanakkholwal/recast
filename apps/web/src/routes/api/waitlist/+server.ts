import { json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { user, waitlist } from "$lib/db/schema";
import type { RequestHandler } from "./$types";

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

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
export const POST: RequestHandler = async ({ request }) => {
	let body: { email?: unknown; source?: unknown; name?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		return json({ ok: false, error: "Invalid JSON" }, { status: 400 });
	}

	const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
	const source = typeof body.source === "string" ? body.source.slice(0, 64) : null;
	const requestedName = typeof body.name === "string" ? body.name.trim().slice(0, 80) : "";

	if (!EMAIL_RE.test(email)) {
		return json({ ok: false, error: "Invalid email" }, { status: 400 });
	}

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
