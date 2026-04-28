import type { PageLoad } from "./$types";

export const ssr = false;

export type Release = {
	tag: string;
	name: string;
	publishedAt: string | null;
	url: string;
	body: string;
	prerelease: boolean;
};

export const load: PageLoad = async ({ fetch, setHeaders }) => {
	setHeaders({ "Cache-Control": "public, max-age=3600" });

	try {
		const res = await fetch(
			"https://api.github.com/repos/kanakkholwal/recast/releases?per_page=20",
		);
		if (!res.ok) throw new Error("Failed to fetch releases");

		const raw = (await res.json()) as Array<{
			tag_name: string;
			name: string | null;
			published_at: string | null;
			html_url: string;
			body: string | null;
			prerelease: boolean;
		}>;

		const releases: Release[] = raw.map((r) => ({
			tag: r.tag_name,
			name: r.name?.trim() || r.tag_name,
			publishedAt: r.published_at,
			url: r.html_url,
			body: (r.body ?? "").trim(),
			prerelease: r.prerelease,
		}));

		return { releases };
	} catch {
		return { releases: [] as Release[] };
	}
};
