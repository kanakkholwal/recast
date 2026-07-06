/**
 * Pure route predicates + brand structured data for the root layout, extracted
 * from `+layout.svelte` so the chromeless/indexable rules and JSON-LD builder
 * stay out of the reactive shell. Take primitives (pathname/origin) so they're
 * trivially callable and testable.
 */

// The dashboard, auth, and waitlist screens ship their own focused shells —
// keep the marketing chrome off them.
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
		pathname === "/accept-invitation" ||
		pathname === "/verify-email" ||
		CHROMELESS_PATHS.has(pathname)
	);
}

// Product surfaces ("the app", not "the website") — these get the branded
// splash + route loading screen. Keep in sync with the inline path check in
// `app.html`, which can't import this (it runs before any bundle loads).
export function isAppArea(pathname: string): boolean {
	return pathname.startsWith("/dashboard") || pathname.startsWith("/share/");
}

// Only the public marketing/tool pages should be indexed; everything else
// (dashboard, admin, auth, onboarding, shares) is marked noindex.
const PUBLIC_PREFIXES = [
	"/features",
	"/extensions",
	"/pricing",
	"/download",
	"/changelog",
	"/privacy-policy",
	"/terms-of-service",
	"/tools",
];

/** True on the home page and the public marketing/tool trees. */
export function isIndexable(pathname: string): boolean {
	return (
		pathname === "/" ||
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
			sameAs: [
				"https://github.com/kanakkholwal/recast",
				"https://x.com/kanakkholwal",
			],
		},
		{
			"@context": "https://schema.org",
			"@type": "WebSite",
			name: "Recast",
			url: origin,
		},
	]);
}
