import { error, json } from "@sveltejs/kit";
import { serverEnv } from "$lib/env/server";
import { isStorageConfigured } from "$lib/storage";
import { runExpirySweep } from "$lib/storage/expire";
import type { RequestHandler } from "./$types";

/**
 * Cron entry point — archives unviewed Free recasts after 14d and
 * hard-deletes them after another 16d. Idempotent and safe to call
 * repeatedly.
 *
 * Auth: `?secret=<CRON_SECRET>` query or `Authorization: Bearer <secret>`.
 * Both checked, either accepted. Returns 503 if R2 isn't configured
 * (no blobs to clean up anyway) and 401 if the secret doesn't match.
 *
 * Schedulable from Vercel cron (vercel.json), Cloudflare Cron Triggers,
 * GitHub Actions, or any external cron via `pnpm cron:expire`
 * (scripts/cron-expire.mjs hits this endpoint).
 *
 * Run frequency: daily is enough — the 14d / 30d boundaries don't need
 * sub-day precision.
 */

function isAuthorized(request: Request, url: URL): boolean {
	const expected = serverEnv().CRON_SECRET;
	if (!expected) return false;

	const fromQuery = url.searchParams.get("secret");
	if (fromQuery && fromQuery === expected) return true;

	const header = request.headers.get("authorization") ?? "";
	const match = /^Bearer\s+(.+)$/i.exec(header);
	if (match && match[1] === expected) return true;

	return false;
}

export const POST: RequestHandler = async ({ request, url }) => {
	if (!isAuthorized(request, url)) error(401, "Unauthorized");
	if (!isStorageConfigured()) error(503, "Storage provider not configured");

	const startedAt = Date.now();
	const result = await runExpirySweep();
	const durationMs = Date.now() - startedAt;

	return json({
		ok: true,
		durationMs,
		...result,
	});
};

// Allow GET too so a simple `curl` or a browser pageload can invoke it
// for ops smoke-testing. Same auth requirement.
export const GET = POST;
