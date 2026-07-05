/** Pure display + permission helpers for the team page. */

export function initials(name: string): string {
	return (
		name
			.split(/\s+/)
			.filter(Boolean)
			.slice(0, 2)
			.map((w) => w[0]!.toUpperCase())
			.join("") || "?"
	);
}

export function capitalize(s: string): string {
	return s ? s[0]!.toUpperCase() + s.slice(1) : s;
}

/** Seats left under the plan's member cap; Infinity when uncapped. */
export function seatsRemaining(memberCap: number, memberCount: number): number {
	return Number.isFinite(memberCap)
		? Math.max(0, memberCap - memberCount)
		: Number.POSITIVE_INFINITY;
}

/** "3 / 5" when capped, bare count when unlimited. */
export function seatsValue(memberCap: number, memberCount: number): string {
	return Number.isFinite(memberCap) ? `${memberCount} / ${memberCap}` : String(memberCount);
}

/** A member is manageable when it isn't you and isn't the owner. */
export function isManageable(
	member: { userId: string; role: string },
	viewer: { userId: string },
	canManage: boolean,
): boolean {
	return canManage && member.userId !== viewer.userId && member.role !== "owner";
}
