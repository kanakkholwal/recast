import { error } from "@sveltejs/kit";
import { eq } from "drizzle-orm";
import { getAuth } from "$lib/auth/server";
import { getDb } from "$lib/db";
import { share } from "$lib/db/schema";
import { loadViewer, resolveShareAccess, type ResolvedShare } from "$lib/share/access";
import { grantCookieName, readGrantedEmail } from "$lib/share/grant";
import { constantTimeEquals, unlockCookieName, unlockToken } from "$lib/share/password";
import { isStorageConfigured, resolvePlaybackUrl, signDownloadUrl } from "$lib/storage";
import type { PageServerLoad } from "./$types";

/**
 * Share page loader.
 *
 * Two paths:
 *   - `params.id === "demo"` → hardcoded Big Buck Bunny payload so the
 *     design surface stays reachable without seeding a row. Always
 *     public, never manageable.
 *   - Real shares → resolve permissions via `resolveShareAccess`, then
 *     decide whether to ship the signed video URL in the SSR payload or
 *     defer it pending a password unlock.
 *
 * `requiresPassword` is true when the share has a passwordHash AND the
 * caller doesn't already carry a valid unlock cookie. In that case
 * `recast.src` is left empty and the page renders the password prompt
 * instead of the player; the client POSTs to /api/share/[id]/unlock to
 * mint the cookie, then refetches /api/share/[id]/video for the URL.
 */

type DemoOrResolved = ResolvedShare & { requiresPassword?: boolean };

const DEMO: DemoOrResolved = {
	ok: true,
	recast: {
		id: "demo",
		title: "Big Buck Bunny",
		description:
			"Mux's public HLS test stream — exercises the adaptive bitrate path through hls.js.",
		src: "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8",
		poster:
			"https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/images/BigBuckBunny.jpg",
		captions: null,
		durationSec: 596,
		width: 1280,
		height: 720,
		sharedBy: "Recast Demo",
		sharedAt: Date.now() - 1000 * 60 * 60 * 6,
	},
	share: {
		slug: "demo",
		visibility: "public",
		organizationId: null,
		ctaLabel: "Try Recast free",
		ctaUrl: "https://recast.li",
		commentsEnabled: true,
		viewsCount: 1280,
		searchable: false,
	},
	canManage: false,
};

type SessionShape = { user: { id: string } };

export const load: PageServerLoad = async ({ params, request, cookies }) => {
	// `customSeo` tells the root layout to suppress its default OG card so the
	// share page's own <SeoMeta> (branded card with this recast's title/owner)
	// is the single authoritative set of og: tags.
	if (params.id === "demo") {
		return { access: DEMO, customSeo: true, signedIn: false };
	}

	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	const signedIn = session != null;

	const viewer = await loadViewer(session?.user.id ?? null);
	// Account-less invitee grant (selected shares). Verified here; the
	// resolver re-checks the email against the allowlist.
	const grantedEmail = await readGrantedEmail(params.id, cookies.get(grantCookieName(params.id)));
	const access: DemoOrResolved = await resolveShareAccess(params.id, viewer, grantedEmail);

	if ("reason" in access && access.reason === "not-found") {
		error(404, "Share link not found");
	}

	// Deny branch — page renders the denial card, no need to sign anything.
	if (!access.ok) return { access, customSeo: true, signedIn };

	// Look up the share's passwordHash separately so `resolveShareAccess`
	// stays focused on visibility. One extra round-trip is fine here —
	// this loader only runs on cold navigations.
	const db = getDb();
	const [s] = await db
		.select({ passwordHash: share.passwordHash })
		.from(share)
		.where(eq(share.slug, params.id))
		.limit(1);

	if (s?.passwordHash) {
		const got = cookies.get(unlockCookieName(params.id));
		const expected = await unlockToken(params.id);
		const unlocked = got != null && constantTimeEquals(got, expected);
		if (!unlocked) {
			return {
				access: {
					...access,
					requiresPassword: true,
					recast: { ...access.recast, src: "" },
				},
				customSeo: true,
				signedIn,
			};
		}
	}

	// Sign the video, poster, and captions in parallel — three independent R2
	// round-trips that used to run in series and blocked the hero for no reason.
	//   • video: stored as a bare key (e.g. "workspace/abc/def.mp4"); anything
	//     starting with http(s) is a legacy/external URL and passes through.
	//   • poster: signed so the <video poster> shows before the first frame
	//     decodes (otherwise the hero is a black box).
	//   • captions: signed so the player can load it as a `<track>` (and the
	//     page's interactive transcript can read cues off it).
	// `resolvePlaybackUrl` no-ops on empty/absolute values and never throws.
	const needsSign = isStorageConfigured() && !/^https?:\/\//.test(access.recast.src);
	const [src, poster, captions] = await Promise.all([
		needsSign
			? signDownloadUrl({ key: access.recast.src, expiresInSeconds: 60 * 60 }).catch((err) => {
					console.error("[share] signDownloadUrl failed", err);
					// Empty src → the page renders "playback unavailable" rather
					// than a broken player.
					return "";
				})
			: Promise.resolve(access.recast.src),
		resolvePlaybackUrl(access.recast.poster, 60 * 60),
		resolvePlaybackUrl(access.recast.captions, 60 * 60),
	]);
	access.recast.src = src;
	access.recast.poster = poster;
	access.recast.captions = captions;

	return { access, customSeo: true, signedIn };
};
