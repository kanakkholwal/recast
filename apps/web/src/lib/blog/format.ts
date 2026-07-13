/**
 * Pure formatting helpers, kept out of `$lib/blog/index.ts` so components can
 * import them without dragging the docvia collection (and every compiled
 * article) into the client bundle.
 */

/** `2026-07-13` -> `Jul 13, 2026`. Empty string on an unparseable input. */
export function formatDate(iso: string): string {
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return "";
	return new Intl.DateTimeFormat("en-US", {
		month: "short",
		day: "numeric",
		year: "numeric",
	}).format(date);
}
