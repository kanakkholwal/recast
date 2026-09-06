/** Pure label/formatting helpers for CloudSignIn. */

/** Title-case a workspace role for the badge ("owner" → "Owner"). */
export function roleLabel(role: string): string {
	return role ? role[0].toUpperCase() + role.slice(1) : "Member";
}

/** Plan name for the badge. */
export function planLabel(plan: string): string {
	if (plan === "pro") return "Pro";
	if (plan === "enterprise") return "Enterprise";
	return "Free";
}

/** "K", "KK", "?": feeds the avatar fallback. */
export function initials(name: string | null, email: string | null): string {
	const source = (name ?? email ?? "").trim();
	if (!source) return "?";
	const parts = source.split(/\s+/).filter(Boolean);
	if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
	return source.slice(0, 2).toUpperCase();
}

/** "May 2026" from an ISO date, or null if absent/unparseable. */
export function formatMemberSince(iso: string | null): string | null {
	if (!iso) return null;
	const d = new Date(iso);
	if (Number.isNaN(d.getTime())) return null;
	return d.toLocaleDateString(undefined, { month: "long", year: "numeric" });
}

/** Group the device code into two halves for legibility ("ABCD-EFGH"). */
export function formatUserCode(code: string): string {
	const clean = code.replace(/-/g, "").toUpperCase();
	if (clean.length <= 4) return clean;
	const half = Math.floor(clean.length / 2);
	return `${clean.slice(0, half)}-${clean.slice(half)}`;
}
