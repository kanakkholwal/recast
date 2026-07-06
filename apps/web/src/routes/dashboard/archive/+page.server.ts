import { and, desc, eq, isNull } from "drizzle-orm";
import { getDb } from "$lib/db";
import { recast } from "$lib/db/schema";
import { QUOTA } from "$lib/db/schema/usage";
import type { PageServerLoad } from "./$types";

const HARD_DELETE_DAYS = QUOTA.free.hardDeleteAfterArchiveDays ?? 16;
const DAY_MS = 24 * 60 * 60 * 1000;

export const load: PageServerLoad = async ({ parent }) => {
	const { activeOrganization } = await parent();
	const rows = await getDb()
		.select({
			id: recast.id,
			title: recast.title,
			durationSec: recast.durationSec,
			sizeBytes: recast.sizeBytes,
			posterUrl: recast.posterUrl,
			archivedAt: recast.archivedAt,
			createdAt: recast.createdAt,
		})
		.from(recast)
		.where(
			and(
				eq(recast.workspaceId, activeOrganization.id),
				eq(recast.status, "archived"),
				isNull(recast.deletedAt),
			),
		)
		.orderBy(desc(recast.archivedAt))
		.limit(100);

	return {
		archived: rows.map((r) => {
			const archivedMs = (r.archivedAt ?? r.createdAt).getTime();
			return {
				id: r.id,
				title: r.title,
				durationSec: r.durationSec,
				sizeBytes: Number(r.sizeBytes),
				posterUrl: r.posterUrl,
				archivedAt: archivedMs,
				deletesAt: archivedMs + HARD_DELETE_DAYS * DAY_MS,
			};
		}),
	};
};
