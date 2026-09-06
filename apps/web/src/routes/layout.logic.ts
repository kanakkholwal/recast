/**
 * Pure route predicates + brand structured data for the root layout, extracted
 * from `+layout.svelte` so the chromeless/indexable rules and JSON-LD builder
 * stay out of the reactive shell. Take primitives (pathname/origin) so they're
 * trivially callable and testable.
 */

// Dashboard, auth and waitlist ship their own focused shells, so keep the marketing chrome off them.
const CHROMELESS_PATHS = new Set([
	"/login",
	"/signup",
	"/forgot-password",
	"/reset-password",
	"/waitlist",
	"/device",
]);

/** True on routes that render their own shell (no marketing navbar/grid). */
export function isChromeless(pathname: string): boolean {
	return (
		pathname.startsWith("/dashboard") ||
		pathname.startsWith("/admin") ||
		pathname.startsWith("/onboarding") ||
		pathname.startsWith("/share/") ||
		// The screenshot editor is a full-height app with its own top bar; its landing page keeps the site chrome.
		pathname.startsWith("/tools/screenshot-editor/edit") ||
		// The playground is one page whose drop surface swaps in-place for the editor, so it owns its whole shell.
		pathname.startsWith("/playground") ||
		pathname === "/accept-invitation" ||
		pathname === "/verify-email" ||
		CHROMELESS_PATHS.has(pathname)
	);
}

/**
 * True on every surface that uses the border-first marketing design system.
 * The complement is the product shells, which still rely on a tonal
 * card-over-canvas lift and must keep the legacy glass tokens.
 */
export function isMarketing(pathname: string): boolean {
	return !(
		pathname.startsWith("/dashboard") ||
		pathname.startsWith("/admin") ||
		pathname.startsWith("/onboarding") ||
		pathname.startsWith("/share/") ||
		pathname.startsWith("/playground") ||
		pathname.startsWith("/tools/screenshot-editor/edit")
	);
}

// Product surfaces get the branded splash; keep in sync with the inline path check in `app.html`, which can't import this.
export function isAppArea(pathname: string): boolean {
	return pathname.startsWith("/dashboard") || pathname.startsWith("/share/");
}

// Only public marketing and tool pages are indexed; everything else is marked noindex.
const PUBLIC_PREFIXES = [
	"/features",
	"/extensions",
	"/pricing",
	"/download",
	"/changelog",
	"/blog",
	"/architecture",
	"/privacy-policy",
	"/terms-of-service",
	"/tools",
];

/** True on the home page and the public marketing/tool trees. */
export function isIndexable(pathname: string): boolean {
	// A real route, but an empty editor is thin content, and `/tools` below would otherwise mark it indexable.
	if (pathname.startsWith("/tools/screenshot-editor/edit")) return false;
	return (
		pathname === "/" ||
		pathname === "/playground" ||
		PUBLIC_PREFIXES.some((p) => pathname === p || pathname.startsWith(`${p}/`))
	);
}

/**
 * Site-wide brand structured data (helps search engines understand the brand
 * and enables sitelinks). Emitted only on indexable pages.
 */
export function buildSiteJsonLd(origin: string): string {
	return JSON.stringify([
		{
			"@context": "https://schema.org",
			"@type": "Organization",
			name: "Recast",
			url: origin,
			sameAs: ["https://github.com/kanakkholwal/recast", "https://x.com/kanakkholwal"],
		},
		{
			"@context": "https://schema.org",
			"@type": "WebSite",
			name: "Recast",
			url: origin,
		},
	]);
}
