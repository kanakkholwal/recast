import { getDb } from "$lib/db";
import { verification } from "$lib/db/schema";
import { publicEnv } from "$lib/env/public";
import { serverEnv } from "$lib/env/server";
import {
	INVITE_TOKEN_TTL_MS,
	resetTokenIdentifier,
	setPasswordUrl,
	stripTrailingSlash,
} from "./invite.logic";

/**
 * Server-side minting of "set your password" links for admin invites and
 * waitlist approvals. See invite.logic.ts for why we write the Better Auth
 * verification row directly instead of calling requestPasswordReset: that
 * endpoint hardcodes the stock reset-password email template, and we want
 * welcome copy.
 */

/** Absolute origin for links. Same precedence Better Auth itself uses. */
export function appOrigin(): string {
	const base = serverEnv().BETTER_AUTH_URL ?? publicEnv().PUBLIC_APP_URL;
	return stripTrailingSlash(base);
}

/**
 * Issues a single-use token that lets `userId` set a password, and returns the
 * absolute URL to email them. Callers are responsible for making sure the user
 * is `active` first — a `pending` user can reach the page but every other auth
 * path stays closed to them.
 */
export async function createSetPasswordLink(userId: string): Promise<string> {
	// 128 bits of opaque entropy: the token is the only thing guarding the account until it is used.
	const token = `${crypto.randomUUID()}${crypto.randomUUID()}`.replace(/-/g, "");
	await getDb()
		.insert(verification)
		.values({
			id: crypto.randomUUID(),
			identifier: resetTokenIdentifier(token),
			value: userId,
			expiresAt: new Date(Date.now() + INVITE_TOKEN_TTL_MS),
		});
	return setPasswordUrl(appOrigin(), token);
}
