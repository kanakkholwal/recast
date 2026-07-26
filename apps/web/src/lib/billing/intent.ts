import { and, eq } from "drizzle-orm";
import { getDb } from "$lib/db";
import { billingCheckoutIntent, member } from "$lib/db/schema";

export type WorkspaceResolution =
	| { ok: true; organizationId: string; seats: number }
	| { ok: false; reason: "no_intent_ambiguous" | "no_owned_workspace" };

/** Owned workspaces, used as the fallback when no intent was recorded. */
async function ownedWorkspaceIds(userId: string): Promise<string[]> {
	const rows = await getDb()
		.select({ organizationId: member.organizationId })
		.from(member)
		.where(and(eq(member.userId, userId), eq(member.role, "owner")));
	return rows.map((r) => r.organizationId);
}

/** Records which workspace this user is buying for, replacing any stale intent. */
export async function recordCheckoutIntent(
	userId: string,
	organizationId: string,
	seats: number,
): Promise<void> {
	await getDb()
		.insert(billingCheckoutIntent)
		.values({ userId, organizationId, seats })
		.onConflictDoUpdate({
			target: billingCheckoutIntent.userId,
			set: { organizationId, seats, createdAt: new Date() },
		});
}

/**
 * Resolves the workspace a Polar payment belongs to. Falls back to the sole
 * owned workspace; refuses to guess when the buyer owns several, since
 * granting the wrong workspace is worse than granting none.
 */
export async function resolveCheckoutWorkspace(
	userId: string,
): Promise<WorkspaceResolution> {
	const db = getDb();
	const [intent] = await db
		.select({
			organizationId: billingCheckoutIntent.organizationId,
			seats: billingCheckoutIntent.seats,
		})
		.from(billingCheckoutIntent)
		.where(eq(billingCheckoutIntent.userId, userId))
		.limit(1);

	if (intent) {
		return {
			ok: true,
			organizationId: intent.organizationId,
			seats: intent.seats,
		};
	}

	const owned = await ownedWorkspaceIds(userId);
	if (owned.length === 1) return { ok: true, organizationId: owned[0]!, seats: 3 };
	return {
		ok: false,
		reason: owned.length === 0 ? "no_owned_workspace" : "no_intent_ambiguous",
	};
}

/** Intents are single-use — a stale one would misroute the next purchase. */
export async function clearCheckoutIntent(userId: string): Promise<void> {
	await getDb()
		.delete(billingCheckoutIntent)
		.where(eq(billingCheckoutIntent.userId, userId));
}
