import { json, type RequestEvent } from "@sveltejs/kit";
import { sql } from "drizzle-orm";
import { getDb } from "$lib/db";
import { rateLimit } from "$lib/db/schema";

/**
 * Durable fixed-window rate limiting backed by Postgres.
 *
 * Why not an in-memory Map: under Vercel Fluid Compute (and any serverless
 * host) instances don't share memory, so an in-memory counter is both
 * non-durable AND a cross-request leak. A single atomic upsert per call keeps
 * the limiter correct across instances at the cost of one cheap write.
 *
 * Fixed-window (not sliding) is deliberate: it's one round-trip and good enough
 * for abuse mitigation on public endpoints (brute-force, email-bomb, view
 * inflation). Tune the limits at the call site, not here.
 */

export interface RateLimitOutcome {
	/** False once the window's request count has exceeded `limit`. */
	ok: boolean;
	/** Seconds until the current window resets (for `Retry-After`). */
	retryAfterSec: number;
}

/**
 * Atomically record a hit against `key` and report whether it's still under
 * `limit` within the last `windowMs`. The upsert resets the window in place
 * when the stored one has already expired, so old rows self-heal without a
 * separate sweep.
 */
export async function consumeRateLimit(
	key: string,
	limit: number,
	windowMs: number,
): Promise<RateLimitOutcome> {
	const db = getDb();
	const now = new Date();
	const newExpiry = new Date(now.getTime() + windowMs);
	// Raw `sql` interpolation doesn't run values through the column's timestamp
	// mapper, so a bare Date reaches postgres-js unconverted and it throws
	// (`ERR_INVALID_ARG_TYPE`). Pass ISO strings; Postgres casts them to the
	// timestamp column for the comparison/assignment.
	const nowIso = now.toISOString();
	const newExpiryIso = newExpiry.toISOString();

	const [row] = await db
		.insert(rateLimit)
		.values({ key, count: 1, expiresAt: newExpiry })
		.onConflictDoUpdate({
			target: rateLimit.key,
			set: {
				// Expired window → start fresh at 1; live window → increment.
				count: sql`case when ${rateLimit.expiresAt} < ${nowIso} then 1 else ${rateLimit.count} + 1 end`,
				expiresAt: sql`case when ${rateLimit.expiresAt} < ${nowIso} then ${newExpiryIso} else ${rateLimit.expiresAt} end`,
			},
		})
		.returning({ count: rateLimit.count, expiresAt: rateLimit.expiresAt });

	const count = row?.count ?? 1;
	const expiresAt = row?.expiresAt ?? newExpiry;
	const retryAfterSec = Math.max(1, Math.ceil((expiresAt.getTime() - now.getTime()) / 1000));

	return { ok: count <= limit, retryAfterSec };
}

export interface EnforceOptions {
	/** Namespace for the counter, e.g. "share-unlock". */
	bucket: string;
	/** Optional per-resource discriminator (e.g. the share slug). */
	id?: string;
	/** Max requests allowed per window. */
	limit: number;
	/** Window length in milliseconds. */
	windowMs: number;
}

/**
 * Convenience wrapper for `+server.ts` handlers: keys the limiter by
 * `bucket:[id:]clientIp`, and returns a ready-to-return 429 `Response` when the
 * caller is over the limit, or `null` when they're clear. Usage:
 *
 *   const limited = await enforceRateLimit(event, { bucket: "share-unlock",
 *     id: params.id, limit: 10, windowMs: 60_000 });
 *   if (limited) return limited;
 */
export async function enforceRateLimit(
	event: Pick<RequestEvent, "getClientAddress">,
	opts: EnforceOptions,
): Promise<Response | null> {
	// `getClientAddress()` throws on adapters that can't determine the address;
	// never let that 500 the handler — fall back to a shared bucket instead.
	let ip: string;
	try {
		ip = event.getClientAddress();
	} catch {
		ip = "unknown";
	}
	const key = `${opts.bucket}:${opts.id ? `${opts.id}:` : ""}${ip}`;

	let ok = true;
	let retryAfterSec = 0;
	try {
		({ ok, retryAfterSec } = await consumeRateLimit(key, opts.limit, opts.windowMs));
	} catch (err) {
		// Fail-open: rate limiting is a best-effort guardrail, so an infrastructure
		// failure (a drifted/missing rate_limit table or its `key` primary key)
		// must never 500 the request it protects. Log loudly so the root cause
		// still gets fixed rather than silently masked.
		console.error(`[rate-limit] consume failed for "${opts.bucket}"; allowing`, err);
		return null;
	}
	if (ok) return null;

	// Mirror the `{ ok: false, reason }` shape the share endpoints already use.
	return json(
		{ ok: false, reason: "rate_limited" },
		{ status: 429, headers: { "Retry-After": String(retryAfterSec) } },
	);
}
