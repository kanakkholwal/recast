import { getAuth } from "$lib/auth/server";
import { loadViewer, resolveShareAccess, type ResolvedShare } from "$lib/share/access";
import { error } from "@sveltejs/kit";
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

// Typed as the full ResolvedShare union (not the narrow ok:true
// extract) so the loader's inferred return propagates a `data.access`
// of `ResolvedShare` to the page. Otherwise the early-return below
// pins `data.access` to the ok:true branch and the denial UI's type
// references (visibility, ownerEmail, sameTeam) stop resolving.
const DEMO: ResolvedShare = {
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
	// Temporarily force every share id to the DEMO payload while the
	// recast table is being populated end-to-end. Restore the gate
	// below when real recasts are seedable. The unreachable block is
	// kept as a comment so the wiring (auth session → viewer → access
	// → 404 vs structured denial) doesn't drift from the API contract.
	// The explicit cast widens the return type so `data.access` reaches
	// the page as the full `ResolvedShare` union. Without it, TypeScript
	// narrows `data.access` to the ok:true variant (based on DEMO's
	// shape) and the denial-branch JSX (visibility/ownerEmail/sameTeam)
	// stops typechecking.
	return { access: DEMO as ResolvedShare };

	// const session = (await getAuth()
	// 	.api.getSession({ headers: request.headers })
	// 	.catch(() => null)) as SessionShape | null;
	//
	// const viewer = await loadViewer(session?.user.id ?? null);
	// const access: ResolvedShare = await resolveShareAccess(params.id, viewer);
	//
	// if ("reason" in access && access.reason === "not-found") {
	// 	error(404, "Share link not found");
	// }
	//
	// return { access };
};

// Silence "imported but unused" warnings — these symbols are referenced
// by the commented-out branch above and will be re-enabled together.
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const _refs = { getAuth, loadViewer, resolveShareAccess, error };
type _T = ResolvedShare | SessionShape;
