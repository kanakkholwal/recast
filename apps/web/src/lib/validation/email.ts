import { z } from "zod";

/**
 * Single source of truth for email validation and normalization.
 *
 * There used to be four copies of `/^[^\s@]+@[^\s@]+\.[^\s@]+$/` (waitlist,
 * auth lookup, share claim, admin invite) plus a fifth route on
 * `z.string().email()`, which meant the same address could be accepted by one
 * endpoint and rejected by another. They're consolidated here on `z.email()`:
 * measured against the old regex it accepts every real-world address and
 * rejects six malformed ones the regex let through (`a@b..com`,
 * `a@-example.com`, `.a@example.com`, `a.@example.com`, `a@example..`,
 * `a@.example.com`).
 */

/**
 * Lowercase + trim. This is the canonical stored form: `share_member.email`
 * and `user.email` are both written normalized, and allowlist lookups compare
 * against it, so normalizing on the way in is what makes those matches work.
 */
export function normalizeEmail(raw: string): string {
	return raw.trim().toLowerCase();
}

export function isValidEmail(raw: string): boolean {
	return z.email().safeParse(normalizeEmail(raw)).success;
}

/**
 * Zod field that normalizes then validates, so every caller stores the same
 * canonical form. Pass a message to match the surrounding endpoint's copy.
 */
export function emailField(message = "Enter a valid email address") {
	return z.string().transform(normalizeEmail).pipe(z.email(message));
}
