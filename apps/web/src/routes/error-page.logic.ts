/**
 * Copy, accent lookups, and next-step data for the global error page, extracted
 * from `+error.svelte` so the status→copy mapping stays out of the reactive
 * shell. `errorCopy` takes primitives (status/message/isServerError) so the
 * component wraps it in a thin `$derived`.
 */

import { BookOpen, Compass, Home, LifeBuoy, MonitorPlay, Search } from "@lucide/svelte";

export type ErrorAccent = "primary" | "amber" | "destructive";

/** One copy block per status face; `accent` keys the tint lookups below. */
export type ErrorCopy = {
	eyebrow: string;
	title: string;
	body: string;
	accent: ErrorAccent;
};

/**
 * Copy for a status code, falling back for anything we don't have a custom face
 * for so the page always renders something sensible — even for a 418. Lookup is
 * by numeric status, so unknown codes fall straight through to the generic
 * fallback.
 */
export function errorCopy(status: number, message: string, isServerError: boolean): ErrorCopy {
	const known: Record<number, ErrorCopy> = {
		404: {
			eyebrow: "404 · Lost in the timeline",
			title: "We can't find that frame.",
			body:
				"The link is broken, the page moved, or the URL has a typo. Let's get you back to something useful.",
			accent: "primary",
		},
		403: {
			eyebrow: "403 · Locked",
			title: "Not yours to see.",
			body:
				"You're signed in, but this corner isn't open to your account. If you think that's a mistake, ping support.",
			accent: "amber",
		},
		401: {
			eyebrow: "401 · Sign in first",
			title: "You'll need an account.",
			body: "This page wants a signed-in user. Sign in and we'll bring you straight back.",
			accent: "amber",
		},
		500: {
			eyebrow: "500 · Recast tripped",
			title: "Something broke on our end.",
			body:
				"That's on us, not you. The error was logged. Try the page again in a moment, or head back to where you were.",
			accent: "destructive",
		},
	};
	return (
		known[status] ?? {
			eyebrow: `${status} · ${isServerError ? "Server error" : "Couldn't load"}`,
			title: "This page didn't render.",
			body: message || "Something went sideways loading the page. Try again, or head home.",
			accent: isServerError ? "destructive" : "primary",
		}
	);
}

/** Ring/chip classes per accent. */
export const ACCENT_RING: Record<ErrorAccent, string> = {
	primary: "ring-primary/25 bg-primary/10 text-primary",
	amber: "ring-amber-500/30 bg-amber-500/15 text-amber-600 dark:text-amber-400",
	destructive: "ring-destructive/30 bg-destructive/12 text-destructive",
};

/** Radial-gradient backdrop colour per accent. */
export const ACCENT_BACKDROP: Record<ErrorAccent, string> = {
	primary: "color-mix(in srgb, var(--color-primary) 10%, transparent)",
	amber: "color-mix(in srgb, oklch(72% 0.18 65) 10%, transparent)",
	destructive: "color-mix(in srgb, var(--color-destructive) 8%, transparent)",
};

// Suggestion tiles — what to try next. Curated rather than site-map-y on
// purpose: 3 anchored next steps reads as helpful, a 12-link tree reads like a
// dead end.
export const suggestions = [
	{ icon: Home, label: "Home", href: "/", desc: "The product overview." },
	{ icon: MonitorPlay, label: "Download", href: "/download", desc: "Get the app for your OS." },
	{ icon: BookOpen, label: "Changelog", href: "/changelog", desc: "What we shipped recently." },
];

/** The header icon for a status: a compass for 404, lifebuoy for 5xx, else search. */
export function pickStatusIcon(status: number, isServerError: boolean) {
	return status === 404 ? Compass : isServerError ? LifeBuoy : Search;
}
