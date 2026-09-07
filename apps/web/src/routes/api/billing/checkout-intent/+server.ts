import { error, json } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { PLANS } from "$lib/billing/catalog";
import { recordCheckoutIntent } from "$lib/billing/intent";
import { getDb } from "$lib/db";
import { member } from "$lib/db/schema";
import { requireUser } from "$lib/workspace/guard";
import type { RequestHandler } from "./$types";

/** Pins the workspace a checkout is for, so the Polar webhook can attribute it. */
export const POST: RequestHandler = async ({ request }) => {
	const user = await requireUser(request);
	const body = (await request.json().catch(() => ({}))) as {
		workspaceId?: string;
		seats?: number;
	};
	const workspaceId = body.workspaceId?.trim();
	if (!workspaceId) error(400, "workspaceId is required");

	// Only an owner can put a workspace on a paid plan; a member must not attach a card to one they merely belong to.
	const [m] = await getDb()
		.select({ role: member.role })
		.from(member)
		.where(and(eq(member.userId, user.id), eq(member.organizationId, workspaceId)))
		.limit(1);
	if (m?.role !== "owner") error(403, "Only the workspace owner can start checkout");

	const requested = Number(body.seats ?? PLANS.pro.seats.included);
	const seats = Math.min(
		Math.max(PLANS.pro.seats.included, Math.floor(requested) || 0),
		PLANS.pro.seats.max ?? Number.MAX_SAFE_INTEGER,
	);

	await recordCheckoutIntent(user.id, workspaceId, seats);
	return json({ ok: true, workspaceId, seats });
};
