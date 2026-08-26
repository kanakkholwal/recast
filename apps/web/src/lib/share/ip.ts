/**
 * Server-side client-IP helpers for the anonymous engagement surface.
 *
 * `resolveClientIp` is header-first (proxy/edge) and wrapped so it can never
 * throw — SvelteKit's `getClientAddress()` throws on adapters that can't
 * determine the address, which would otherwise surface as an opaque 500 on the
 * endpoints that call it.
 *
 * `reactorKey` turns the reactor into a stable, non-reversible dedup identity:
 * one reaction per (share, reactor). Signed-in viewers key on their account id
 * (stable across IPs); anonymous viewers key on IP (one IP → one reaction),
 * falling back to the browser session when the IP can't be resolved. The raw
 * id/IP is never stored or returned — only a salted hash — so the client can't
 * learn who reacted.
 */
import { createHash } from "node:crypto";
import { serverEnv } from "$lib/env/server";

export function resolveClientIp(request: Request, getClientAddress: () => string): string {
	const forwarded = request.headers.get("x-forwarded-for");
	if (forwarded) {
		// `x-forwarded-for: client, proxy1, proxy2` — the client is first.
		const first = forwarded.split(",")[0]?.trim();
		if (first) return first;
	}
	const direct = request.headers.get("cf-connecting-ip") ?? request.headers.get("x-real-ip");
	if (direct) return direct.trim();
	try {
		return getClientAddress();
	} catch {
		return "";
	}
}

export function reactorKey(opts: {
	userId?: string | null;
	ip: string;
	sessionId: string;
}): string {
	// Account id first (stable, and never leaves the server), then IP, then the
	// browser session so anonymous viewers with an unresolved IP don't all
	// collapse onto one shared token.
	const basis = opts.userId
		? `uid:${opts.userId}`
		: opts.ip
			? `ip:${opts.ip}`
			: `sid:${opts.sessionId}`;
	return createHash("sha256").update(`${serverEnv().BETTER_AUTH_SECRET}:${basis}`).digest("hex");
}
