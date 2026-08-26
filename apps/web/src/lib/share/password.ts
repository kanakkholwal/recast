import { serverEnv } from "$lib/env/server";

/**
 * Share-link password protection.
 *
 * Threat model is **link-leak**, not offline attack: the link is the
 * primary capability; the password adds a "you must also know this string"
 * layer on top. SHA-256 with a per-share salt is fine — we're not
 * protecting a credential database. Stays edge-runtime-compatible
 * (Web Crypto only, no bcrypt native dep).
 *
 * Hash format: `<32-hex-salt>:<64-hex-digest>`. The digest is
 * SHA-256(salt + password).
 *
 * Unlock cookie: once the user submits the right password we set
 * `recast_unlock_<slug>=<hmac>` where the hmac is keyed by
 * BETTER_AUTH_SECRET. The token is deterministic per slug so the same
 * browser can unlock once and watch repeatedly without re-entering. Token
 * invalidates if the auth secret rotates — acceptable, since rotation
 * is rare and re-prompting once is fine.
 */

const HASH_SEPARATOR = ":";
const UNLOCK_COOKIE_PREFIX = "recast_unlock_";

export function unlockCookieName(slug: string): string {
	return UNLOCK_COOKIE_PREFIX + slug;
}

function hex(bytes: Uint8Array): string {
	let out = "";
	for (const b of bytes) out += b.toString(16).padStart(2, "0");
	return out;
}

async function sha256Hex(input: string): Promise<string> {
	const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(input));
	return hex(new Uint8Array(digest));
}

/**
 * Hash a password for `share.passwordHash`. Empty / undefined returns
 * null so callers can store "no password" without branching.
 */
export async function hashSharePassword(
	plaintext: string | undefined | null,
): Promise<string | null> {
	if (!plaintext) return null;
	const saltBytes = crypto.getRandomValues(new Uint8Array(16));
	const salt = hex(saltBytes);
	const digest = await sha256Hex(salt + plaintext);
	return `${salt}${HASH_SEPARATOR}${digest}`;
}

/**
 * Constant-time-ish verify. `crypto.subtle` doesn't expose a timing-safe
 * compare, so we compare hex strings of equal length — close enough for
 * the link-leak threat model and consistent with how short-circuiting
 * works on a length mismatch.
 */
export async function verifySharePassword(
	plaintext: string,
	stored: string | null | undefined,
): Promise<boolean> {
	if (!stored) return false;
	const [salt, expected] = stored.split(HASH_SEPARATOR);
	if (!salt || !expected) return false;
	const actual = await sha256Hex(salt + plaintext);
	if (actual.length !== expected.length) return false;
	let mismatch = 0;
	for (let i = 0; i < actual.length; i++) {
		mismatch |= actual.charCodeAt(i) ^ expected.charCodeAt(i);
	}
	return mismatch === 0;
}

/**
 * Deterministic HMAC-SHA-256 of the slug, hex-encoded and truncated to
 * 32 chars. Used as the value of the unlock cookie — server recomputes
 * it on every /video request and compares; no DB hit needed.
 *
 * Keyed by BETTER_AUTH_SECRET (already required env, already long enough)
 * so the cookie can't be forged without that secret. Rotating the secret
 * invalidates all outstanding unlock cookies, which is the desired
 * behavior for credential rotation.
 */
export async function unlockToken(slug: string): Promise<string> {
	const key = await crypto.subtle.importKey(
		"raw",
		new TextEncoder().encode(serverEnv().BETTER_AUTH_SECRET),
		{ name: "HMAC", hash: "SHA-256" },
		false,
		["sign"],
	);
	const sig = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(`unlock:${slug}`));
	return hex(new Uint8Array(sig)).slice(0, 32);
}

/** Constant-time compare for the unlock token check. */
export function constantTimeEquals(a: string, b: string): boolean {
	if (a.length !== b.length) return false;
	let mismatch = 0;
	for (let i = 0; i < a.length; i++) {
		mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
	}
	return mismatch === 0;
}
