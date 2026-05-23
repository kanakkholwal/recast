import { error } from "@sveltejs/kit";
import { getAuth } from "$lib/auth/server";
import { loadViewer, resolveShareAccess, type ResolvedShare } from "$lib/share/access";
import type { PageServerLoad } from "./$types";

/**
 * Share page loader.
 *
 * Real shares live in `share` keyed by slug; the loader pulls the recast
 * row, evaluates viewer permissions, and returns either the player
 * payload OR a structured denial so the page can render a "no access"
 * fallback (vs. SvelteKit's bare error page).
 *
 * `params.id === "demo"` short-circuits to the hardcoded Big Buck Bunny
 * stream so the design surface stays reachable without seeding a row.
 * Demo is always public and never manageable.
 */

const DEMO: Extract<ResolvedShare, { ok: true }> = {
	ok: true,
	recast: {
		id: "demo",
		title: "Big Buck Bunny",
		description:
			"Mux's public HLS test stream — exercises the adaptive bitrate path through hls.js.",
		src: "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8",
		poster:
			"https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/images/BigBuckBunny.jpg",
		durationSec: 596,
		sharedBy: "Recast Demo",
		sharedAt: Date.now() - 1000 * 60 * 60 * 6,
	},
	share: {
		slug: "demo",
		visibility: "public",
		organizationId: null,
	},
	canManage: false,
};

type SessionShape = { user: { id: string } };

export const load: PageServerLoad = async ({ params, request }) => {
	if (params.id === "demo") {
		return { access: DEMO };
	}

	const session = (await getAuth()
		.api.getSession({ headers: request.headers })
		.catch(() => null)) as SessionShape | null;

	const viewer = await loadViewer(session?.user.id ?? null);
	const access = await resolveShareAccess(params.id, viewer);

	// `not-found` is a true 404 — there's nothing to render. `denied`
	// returns 200 so the page can show the access-denied card with the
	// owner contact / request-access affordance.
	if (!access.ok && access.reason === "not-found") {
		error(404, "Share link not found");
	}

	return { access };
};
