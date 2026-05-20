import { getAuth } from "$lib/auth/server";
import type { RequestHandler } from "./$types";

/**
 * Catch-all for Better Auth + every plugin it mounts:
 *   - email/password endpoints
 *   - magic link
 *   - Polar checkout, customer portal, webhooks (under /api/auth/polar/...)
 */
const handler: RequestHandler = ({ request }) => getAuth().handler(request);

export const GET = handler;
export const POST = handler;
export const PUT = handler;
export const PATCH = handler;
export const DELETE = handler;
export const OPTIONS = handler;
