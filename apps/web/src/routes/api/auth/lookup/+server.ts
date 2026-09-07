import { json } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { user } from "$lib/db/schema";
import { enforceRateLimit } from "$lib/server/rate-limit";
import { isValidEmail, normalizeEmail } from "$lib/validation/email";
import type { RequestHandler } from "./$types";

/**
 * Pre-flight email lookup shared by /login and /signup, so each form can answer
 * a dead end with a link instead of a cryptic toast:
 *
 *   - `unknown`  → no row at all. Login offers sign-up; sign-up proceeds.
 *   - `pending`  → a waitlist-era row that never set a password. It can sign in
 *                  by magic link now, so only the password tab heads it off;
 *                  sign-up sends them to /login.
 *   - `active`   → a real account. Login proceeds; sign-up sends them to /login.
 *
 * Banned users intentionally surface as `active` — Better Auth's own sign-in
 * path returns the ban reason, which is the right message to show.
 *
 * Exposing existence is a deliberate trade-off. Both the sign-up and sign-in
 * endpoints already leak it through their error messages, so the rate limit
 * below (not secrecy) is what bounds enumeration.
 */
export const POST: RequestHandler = async ({ request, getClientAddress }) => {
	// Bound the existence oracle with a per-IP cap real login forms never reach but an enumeration script does.
	const limited = await enforceRateLimit(
		{ getClientAddress },
		{ bucket: "auth-lookup", limit: 20, windowMs: 60_000 },
	);
	if (limited) return limited;

	let body: { email?: unknown } = {};
	try {
		body = (await request.json()) as typeof body;
	} catch {
		return json({ status: "invalid" as const });
	}
	const email = typeof body.email === "string" ? normalizeEmail(body.email) : "";
	if (!isValidEmail(email)) return json({ status: "invalid" as const });

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
