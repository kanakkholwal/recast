import { json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { user } from "$lib/db/schema";
import type { RequestHandler } from "./$types";

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

/**
 * Pre-flight email lookup for the login form. Production-only signup means
 * "log in" should never silently create an account, so the form needs to
 * differentiate three cases before calling auth:
 *
 *   - `unknown`  → no row at all, route them to /waitlist instead of a
 *                  cryptic "invalid credentials" toast.
 *   - `pending`  → on the waitlist, magic-link is suppressed server-side
 *                  ([isOnWaitlist] short-circuits sendMagicLink). Tell them
 *                  explicitly so they don't keep retrying.
 *   - `active`   → proceed with the actual auth call.
 *
 * Banned users intentionally surface as `active` here — Better Auth's own
 * sign-in path returns the ban reason, which is the right message to show.
 *
 * Exposing existence is a deliberate trade-off: we already publish a public
 * waitlist endpoint that takes any email, so an attacker can already probe
 * registration. The UX win outweighs the marginal info leak.
 */
export const POST: RequestHandler = async ({ request }) => {
	let body: { email?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		return json({ status: "invalid" as const });
	}
	const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
	if (!EMAIL_RE.test(email)) return json({ status: "invalid" as const });

	const db = getDb();
	const [row] = await db
		.select({ status: user.status })
		.from(user)
		.where(eq(user.email, email))
		.limit(1);

	if (!row) return json({ status: "unknown" as const });
	if (row.status === "pending") return json({ status: "pending" as const });
	return json({ status: "active" as const });
};
