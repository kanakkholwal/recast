/**
 * Mermaid derives its whole palette by doing colour maths on these, through a
 * library that throws on anything it cannot parse. `currentColor` and
 * `transparent` both threw, which silently killed every diagram on the site: the
 * render rejected and each block fell back to its own source.
 *
 * So these are real, neutral values that read on either theme, and the actual
 * theming is the token-driven CSS in `MermaidDiagram.svelte`.
 */
export const MERMAID_THEME_VARIABLES = {
	fontFamily: 'ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
	fontSize: "14px",
	primaryColor: "#8a8a8a",
	primaryTextColor: "#4a4a4a",
	primaryBorderColor: "#8a8a8a",
	lineColor: "#8a8a8a",
	secondaryColor: "#9a9a9a",
	tertiaryColor: "#aaaaaa",
} as const;

/** Values mermaid's colour maths cannot parse, whatever CSS thinks of them. */
const UNPARSEABLE = /^(currentcolor|transparent|inherit|initial|unset|var\()/i;

/** Theme values that would make mermaid throw before it draws anything. */
export function unparseableThemeColors(
	variables: Record<string, string> = MERMAID_THEME_VARIABLES,
): string[] {
	return Object.entries(variables)
		.filter(([key]) => key.toLowerCase().includes("color"))
		.filter(([, value]) => UNPARSEABLE.test(value.trim()))
		.map(([key]) => key);
}
