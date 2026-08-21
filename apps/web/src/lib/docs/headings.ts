/** A `##` heading, for the on-page contents rail. */
export interface DocHeading {
	depth: number;
	text: string;
	id: string;
}

/**
 * The top-level headings docvia derived from the markdown.
 *
 * Only `##`: `###` and deeper would turn the rail into a second copy of the
 * document. A heading with no anchor is dropped, because there is nothing to
 * link it to.
 */
export function docHeadings(value: unknown): DocHeading[] {
	if (!Array.isArray(value)) return [];
	return value
		.map((entry) => entry as Partial<DocHeading>)
		.filter(
			(entry) => Number(entry.depth) === 2 && typeof entry.id === "string" && entry.id.length > 0,
		)
		.map((entry) => ({ depth: 2, text: String(entry.text ?? ""), id: String(entry.id) }));
}
