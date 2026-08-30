/**
 * Pure user-agent / referrer normalization shared by the view-ingest endpoint
 * (where it runs at write time) and the activity loaders (where it back-fills
 * historical rows that predate the `device` column). Deliberately dependency-
 * free and heuristic — we only need a coarse mobile/tablet/desktop class and a
 * clean referrer host, not a full UA database.
 */

export type DeviceClass = "mobile" | "tablet" | "desktop";

/**
 * Coarse device class from a user-agent string. Tablet is checked before
 * mobile because tablet UAs (iPad, Android tablets) also contain "mobile"-ish
 * tokens. Unknown / empty UAs fall back to `desktop` (the safest default for
 * an embed that loaded a full player).
 */
export function deviceFromUA(ua: string | null | undefined): DeviceClass {
	if (!ua) return "desktop";
	const s = ua.toLowerCase();

	// Tablets first: iPadOS 13+ reports Macintosh, so catch the touch-Mac signal, and Android tablets lack 'mobile'.
	if (
		/ipad|tablet|kindle|silk|playbook/.test(s) ||
		(/android/.test(s) && !/mobile/.test(s)) ||
		(/macintosh/.test(s) && /mobile|touch/.test(s))
	) {
		return "tablet";
	}

	if (/mobi|iphone|ipod|android.*mobile|windows phone|blackberry|bb10|opera mini/.test(s)) {
		return "mobile";
	}

	return "desktop";
}

/**
 * Reduce a raw `document.referrer` to a bare hostname (lower-cased, `www.`
 * stripped) for grouping. Returns null for empty / same-origin-stripped /
 * unparseable values so direct traffic doesn't pollute the breakdown.
 */
export function referrerHost(
	raw: string | null | undefined,
	selfOrigin?: string | null,
): string | null {
	if (!raw || typeof raw !== "string") return null;
	const trimmed = raw.trim().slice(0, 2048);
	if (!trimmed) return null;
	try {
		const url = new URL(trimmed);
		if (url.protocol !== "http:" && url.protocol !== "https:") return null;
		const host = url.hostname.toLowerCase().replace(/^www\./, "");
		if (!host) return null;
		// Drop self-referrals (in-app navigation, the page reloading itself): they aren't an acquisition source.
		if (selfOrigin) {
			try {
				const selfHost = new URL(selfOrigin).hostname.toLowerCase().replace(/^www\./, "");
				if (host === selfHost) return null;
			} catch {
				/* ignore bad selfOrigin */
			}
		}
		return host.slice(0, 128);
	} catch {
		return null;
	}
}
