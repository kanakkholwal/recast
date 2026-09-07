/** Up-to-two-letter uppercase initials for a team name; "T" when empty. */
export function initials(name: string): string {
	return (
		name
			.split(/\s+/)
			.filter(Boolean)
			.slice(0, 2)
			.map((w) => w[0].toUpperCase())
			.join("") || "T"
	);
}
