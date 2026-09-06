import { browser } from "$app/environment";

/**
 * Web consent state for the cookieless-by-default posture.
 *
 * Basic product analytics run for everyone in memory-only / anonymous mode (no
 * persistent identifiers, no banner required). The banner is purely about
 * *upgrading*: "Accept" unlocks session replay + a persistent profile; "Decline"
 * keeps everything memory-only. The choice is remembered in a first-party cookie
 * so the banner shows once.
 */

const COOKIE = "recast_consent";
const ONE_YEAR = 60 * 60 * 24 * 365;

export type ConsentChoice = "accepted" | "declined" | null;

function readCookie(): ConsentChoice {
	if (!browser) return null;
	const match = document.cookie.match(/(?:^|;\s*)recast_consent=([^;]+)/);
	const value = match?.[1];
	return value === "accepted" || value === "declined" ? value : null;
}

function writeCookie(choice: "accepted" | "declined") {
	if (!browser) return;
	// biome-ignore lint/suspicious/noDocumentCookie: cookieStore is Chromium-only; document.cookie is the portable write.
	document.cookie = `${COOKIE}=${choice}; path=/; max-age=${ONE_YEAR}; samesite=lax`;
}

function createConsentStore() {
	let choice = $state<ConsentChoice>(readCookie());

	return {
		get choice() {
			return choice;
		},
		/** Show the banner only until the visitor has made a choice. */
		get needsBanner() {
			return choice === null;
		},
		get hasAccepted() {
			return choice === "accepted";
		},
		accept() {
			choice = "accepted";
			writeCookie("accepted");
		},
		decline() {
			choice = "declined";
			writeCookie("declined");
		},
	};
}

export const webConsent = createConsentStore();
