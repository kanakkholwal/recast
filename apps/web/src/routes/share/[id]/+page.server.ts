import { error } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";

/**
 * Public share page demo.
 *
 * In v1 we don't have a recasts DB table — once we do, this loader will
 * look up by `params.id`, check share permissions, and return the row.
 * For now it serves a single hardcoded demo (Big Buck Bunny, hosted by
 * Mux's public test-streams bucket) when `id === "demo"`, and 404s
 * otherwise. The point of the route is to validate the player surface
 * end-to-end on a real HLS stream before we wire the data layer.
 */

const DEMO_RECASTS: Record<
	string,
	{
		title: string;
		description: string;
		// HLS (.m3u8) URL — RecastPlayer auto-routes this through hls.js.
		src: string;
		poster: string;
		// Length used by the engagement model + footer chip.
		durationSec: number;
		sharedBy: string;
		sharedAt: number;
	}
> = {
	demo: {
		title: "Big Buck Bunny",
		description:
			"Mux's public HLS test stream — exercises the adaptive bitrate path through hls.js.",
		src: "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8",
		poster:
			"https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/images/BigBuckBunny.jpg",
		durationSec: 596,
		sharedBy: "Recast Demo",
		sharedAt: Date.now() - 1000 * 60 * 60 * 6, // 6h ago
	},
};

export const load: PageServerLoad = async ({ params }) => {
	const recast = DEMO_RECASTS[params.id];
	if (!recast) error(404, "Share link not found");
	return { recast };
};
