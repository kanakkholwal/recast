import { getDb } from "$lib/db";
import { auditLog, type AuditAction } from "$lib/db/schema";

/**
 * Records a single audit entry. Call this *after* the underlying admin
 * action succeeded; if the insert fails we log and swallow — losing an
 * audit record shouldn't bubble up as a 500 to the admin who just
 * successfully banned someone.
 */
export async function logAudit(args: {
	actorId: string;
	action: AuditAction;
	targetUserId?: string | null;
	metadata?: Record<string, unknown>;
}): Promise<void> {
	try {
		await getDb().insert(auditLog).values({
			id: crypto.randomUUID(),
			actorId: args.actorId,
			action: args.action,
			targetUserId: args.targetUserId ?? null,
			metadata: args.metadata ?? null,
		});
	} catch (err) {
		console.error("[audit] failed to record entry", { args, err });
	}
}
