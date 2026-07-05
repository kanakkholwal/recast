/**
 * Builds the takumi-generated OG image URL. Takes plain `page.url` values as
 * args so it stays free of reactive state.
 */
export function buildOgUrl(
	origin: string,
	title: string,
	description: string,
	eyebrow?: string,
): string {
	const params = new URLSearchParams({ title, description });
	if (eyebrow) params.set("eyebrow", eyebrow);
	return `${origin}/api/og?${params.toString()}`;
}
