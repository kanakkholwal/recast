import { normalizeEmail } from "$lib/validation/email";

/**
 * Pure helpers for admin invites and waitlist approvals. Kept free of `$app/*`
 * and the DB so the vitest suite (node env, no SvelteKit plugins) can cover the
 * URL and validation rules that are easy to get subtly wrong.
 *
 * Email validation lives in $lib/validation/email, not here.
 */

/**
 * Invite and approval links are long-lived on purpose. Better Auth's own
 * password-reset token expires in an hour, which is right for "I forgot my
 * password" and wrong for "an admin approved you while you were asleep".
 */
export const INVITE_TOKEN_TTL_MS = 7 * 24 * 60 * 60 * 1000;

/**
 * Better Auth looks the token up under this exact identifier
 * (see better-auth/dist/api/routes/password.mjs — `reset-password:${token}`).
 * Minting the row ourselves lets us send our own copy instead of the stock
 * "reset your password" template, while still going through Better Auth's
 * verified reset endpoint. That endpoint creates a `credential` account when
 * the user has none, which is what makes this work for someone who has never
 * had a password.
 */
export function resetTokenIdentifier(token: string): string {
	return `reset-password:${token}`;
}

/**
 * Links to the app's own /reset-password page rather than Better Auth's
 * /reset-password/:token redirect hop, which requires a callbackURL that
 * passes originCheck. The page reads `?token=` directly.
 */
export function setPasswordUrl(origin: string, token: string): string {
	return `${stripTrailingSlash(origin)}/reset-password?token=${encodeURIComponent(token)}`;
}

export function stripTrailingSlash(url: string): string {
	return url.replace(/\/+$/, "");
}

/**
 * Admins may leave the name blank — fall back to the local part so the user
 * row and the email greeting have something human. Matches what the public
 * waitlist endpoint does.
 */
export function inviteDisplayName(name: string, email: string): string {
	const trimmed = name.trim();
	if (trimmed) return trimmed.slice(0, 80);
	return normalizeEmail(email).split("@")[0] ?? "there";
}

/** First name for email greetings, or null when there's nothing usable. */
export function firstNameOf(name: string | null | undefined): string | null {
	const first = name?.trim().split(/\s+/)[0];
	return first ? first : null;
}
