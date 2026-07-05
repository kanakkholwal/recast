/**
 * Slug base for a team name: lowercased, non-alphanumerics collapsed to dashes,
 * edges trimmed, falling back to "team". The caller appends a random suffix so
 * the unique slug index never collides — kept out of here so this stays
 * deterministic.
 */
export function slugifyBase(name: string): string {
	return (
		name
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/(^-|-$)/g, "") || "team"
	);
}
