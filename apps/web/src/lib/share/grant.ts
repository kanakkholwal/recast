import { serverEnv } from "$lib/env/server";
import { normalizeEmail } from "$lib/validation/email";

/**
 * Account-less access grants for `selected` (invite-only) shares.
 *
 * Selected shares list invitees by email in `share_member`. A signed-in user
 * whose email is on the list is allowed by the normal session path. But many
 * invitees don't have — and shouldn't need — a Recast account (sign-up is
 * waitlist-gated). This module gives them a **share-scoped** capability
 * instead of a global session:
 *
 *   1. Invitee enters their email on the share page → `POST .../claim`.
 *   2. If that email is on the share's allowlist, we email a verify link
 *      carrying `grantToken(slug, email)` (an HMAC, not a secret to store).
 *   3. Clicking the link → `GET .../claim/verify` recomputes the token,
 *      and on a match sets an httpOnly grant cookie, then redirects back.
 *   4. Every gated request re-derives the granted email from the cookie AND
 *      re-checks it against `share_member` — so removing an invitee revokes
 *      access immediately, with no token to invalidate.
 *
 * The token is deterministic per (slug, email), mirroring the password
 * unlock-token design in `./password.ts`: the link is the capability, and
 * membership is the authority. Keyed by BETTER_AUTH_SECRET, so it can't be
 * forged and rotates with the secret.
 */

const GRANT_COOKIE_PREFIX = "recast_grant_";

export function grantCookieName(slug: string): string {
	return GRANT_COOKIE_PREFIX + slug;
}

/**
 * Re-exported so the many existing `from "$lib/share/grant"` call sites keep
 * working. The canonical form stored in `share_member.email` is now defined
 * once in $lib/validation/email, alongside the validator that produces it.
 */
export { normalizeEmail };

function hex(bytes: Uint8Array): string {
	let out = "";
	for (const b of bytes) out += b.toString(16).padStart(2, "0");
	return out;
}

/** Deterministic HMAC-SHA-256 over (slug, normalized email), hex, 32 chars. */
export async function grantToken(slug: string, email: string): Promise<string> {
	const key = await crypto.subtle.importKey(
		"raw",
		new TextEncoder().encode(serverEnv().BETTER_AUTH_SECRET),
		{ name: "HMAC", hash: "SHA-256" },
		false,
		["sign"],
	);
	const sig = await crypto.subtle.sign(
		"HMAC",
		key,
		new TextEncoder().encode(`grant:${slug}:${normalizeEmail(email)}`),
	);
	return hex(new Uint8Array(sig)).slice(0, 32);
}

export function constantTimeEquals(a: string, b: string): boolean {
	if (a.length !== b.length) return false;
	let mismatch = 0;
	for (let i = 0; i < a.length; i++) {
		mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
	}
	return mismatch === 0;
}

// URL-safe base64 of the (ASCII) email so it survives a cookie value next to
// the hex token, separated by a "." that base64url never emits.
function b64urlEncode(s: string): string {
	return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function b64urlDecode(s: string): string | null {
	try {
		const padded = s.replace(/-/g, "+").replace(/_/g, "/");
		return atob(padded);
	} catch {
		return null;
	}
}

/** Cookie value binding the verified email to its token: `<b64url(email)>.<token>`. */
export async function grantCookieValue(slug: string, email: string): Promise<string> {
	const token = await grantToken(slug, email);
	return `${b64urlEncode(normalizeEmail(email))}.${token}`;
}

/**
 * Parse a grant cookie and return the email it certifies for `slug`, or null
 * if absent / malformed / forged. Callers MUST still verify the returned
 * email is on the share's allowlist — this only proves the cookie is genuine.
 */
export async function readGrantedEmail(
	slug: string,
	cookieValue: string | undefined,
): Promise<string | null> {
	if (!cookieValue) return null;
	const dot = cookieValue.lastIndexOf(".");
	if (dot <= 0) return null;
	const email = b64urlDecode(cookieValue.slice(0, dot));
	const token = cookieValue.slice(dot + 1);
	if (!email) return null;
	const expected = await grantToken(slug, email);
	if (!constantTimeEquals(token, expected)) return null;
	return normalizeEmail(email);
}
