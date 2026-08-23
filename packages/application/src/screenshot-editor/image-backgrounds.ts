/** Bundled raster background galleries + light/shadow overlays (in each app's
 * static/screenshot-assets). Originally authored by screenshot-studio
 * (Apache-2.0, credited in NOTICE.md). Apple/Raycast wallpaper packs are
 * deliberately NOT bundled (third-party IP). */

export interface ImageBackground {
	id: string;
	url: string;
}

export interface ImageBackgroundCategory {
	id: string;
	label: string;
	images: ImageBackground[];
}

/** 45 images across 4 categories. Selecting one paints the
 * stage backdrop via a `url(...) center / cover` CSS background. */
export const IMAGE_BACKGROUND_CATEGORIES: ImageBackgroundCategory[] = [
	{
		id: "radiant",
		label: "Radiant",
		images: [
			{ id: "img-radiant-radiant1", url: "/screenshot-assets/radiant/radiant1.jpg" },
			{ id: "img-radiant-radiant10", url: "/screenshot-assets/radiant/radiant10.jpg" },
			{ id: "img-radiant-radiant2", url: "/screenshot-assets/radiant/radiant2.jpg" },
			{ id: "img-radiant-radiant3", url: "/screenshot-assets/radiant/radiant3.jpg" },
			{ id: "img-radiant-radiant4", url: "/screenshot-assets/radiant/radiant4.jpg" },
			{ id: "img-radiant-radiant5", url: "/screenshot-assets/radiant/radiant5.jpg" },
			{ id: "img-radiant-radiant6", url: "/screenshot-assets/radiant/radiant6.jpg" },
			{ id: "img-radiant-radiant8", url: "/screenshot-assets/radiant/radiant8.jpg" },
			{ id: "img-radiant-radiant9", url: "/screenshot-assets/radiant/radiant9.jpg" },
		],
	},
	{
		id: "mesh",
		label: "Mesh",
		images: [
			{ id: "img-mesh-Astra", url: "/screenshot-assets/mesh/Astra.webp" },
			{ id: "img-mesh-Bliss", url: "/screenshot-assets/mesh/Bliss.webp" },
			{ id: "img-mesh-Burst", url: "/screenshot-assets/mesh/Burst.webp" },
			{ id: "img-mesh-Dusk", url: "/screenshot-assets/mesh/Dusk.webp" },
			{ id: "img-mesh-Flash", url: "/screenshot-assets/mesh/Flash.webp" },
			{ id: "img-mesh-Ghost", url: "/screenshot-assets/mesh/Ghost.webp" },
			{ id: "img-mesh-Helix", url: "/screenshot-assets/mesh/Helix.webp" },
			{ id: "img-mesh-Horizon", url: "/screenshot-assets/mesh/Horizon.webp" },
			{ id: "img-mesh-Peak", url: "/screenshot-assets/mesh/Peak.webp" },
			{ id: "img-mesh-mesh1", url: "/screenshot-assets/mesh/mesh1.webp" },
			{ id: "img-mesh-mesh2", url: "/screenshot-assets/mesh/mesh2.webp" },
			{ id: "img-mesh-mesh3", url: "/screenshot-assets/mesh/mesh3.webp" },
			{ id: "img-mesh-mesh4", url: "/screenshot-assets/mesh/mesh4.webp" },
			{ id: "img-mesh-mesh5", url: "/screenshot-assets/mesh/mesh5.webp" },
			{ id: "img-mesh-mesh6", url: "/screenshot-assets/mesh/mesh6.webp" },
			{ id: "img-mesh-mesh7", url: "/screenshot-assets/mesh/mesh7.webp" },
			{ id: "img-mesh-mesh8", url: "/screenshot-assets/mesh/mesh8.webp" },
		],
	},
	{
		id: "pattern",
		label: "Patterns",
		images: [
			{ id: "img-pattern-1", url: "/screenshot-assets/pattern/1.webp" },
			{ id: "img-pattern-10", url: "/screenshot-assets/pattern/10.webp" },
			{ id: "img-pattern-11", url: "/screenshot-assets/pattern/11.webp" },
			{ id: "img-pattern-2", url: "/screenshot-assets/pattern/2.webp" },
			{ id: "img-pattern-3", url: "/screenshot-assets/pattern/3.webp" },
			{ id: "img-pattern-4", url: "/screenshot-assets/pattern/4.webp" },
			{ id: "img-pattern-5", url: "/screenshot-assets/pattern/5.webp" },
			{ id: "img-pattern-6", url: "/screenshot-assets/pattern/6.webp" },
			{ id: "img-pattern-7", url: "/screenshot-assets/pattern/7.webp" },
			{ id: "img-pattern-8", url: "/screenshot-assets/pattern/8.webp" },
			{ id: "img-pattern-9", url: "/screenshot-assets/pattern/9.webp" },
		],
	},
	{
		id: "paper",
		label: "Paper",
		images: [
			{ id: "img-paper-01", url: "/screenshot-assets/paper/01.webp" },
			{ id: "img-paper-02", url: "/screenshot-assets/paper/02.webp" },
			{ id: "img-paper-03", url: "/screenshot-assets/paper/03.webp" },
			{ id: "img-paper-21", url: "/screenshot-assets/paper/21.webp" },
			{ id: "img-paper-26", url: "/screenshot-assets/paper/26.webp" },
			{ id: "img-paper-27", url: "/screenshot-assets/paper/27.webp" },
			{ id: "img-paper-31", url: "/screenshot-assets/paper/31.webp" },
			{ id: "img-paper-47", url: "/screenshot-assets/paper/47.webp" },
		],
	},
];

/** 19 soft light/shadow overlays, added as low-opacity image overlays. */
export const OVERLAY_SHADOWS: ImageBackground[] = [
	{ id: "shadow-001", url: "/screenshot-assets/overlay-shadow/001.webp" },
	{ id: "shadow-002", url: "/screenshot-assets/overlay-shadow/002.webp" },
	{ id: "shadow-007", url: "/screenshot-assets/overlay-shadow/007.webp" },
	{ id: "shadow-017", url: "/screenshot-assets/overlay-shadow/017.webp" },
	{ id: "shadow-019", url: "/screenshot-assets/overlay-shadow/019.webp" },
	{ id: "shadow-023", url: "/screenshot-assets/overlay-shadow/023.webp" },
	{ id: "shadow-031", url: "/screenshot-assets/overlay-shadow/031.webp" },
	{ id: "shadow-037", url: "/screenshot-assets/overlay-shadow/037.webp" },
	{ id: "shadow-041", url: "/screenshot-assets/overlay-shadow/041.webp" },
	{ id: "shadow-050", url: "/screenshot-assets/overlay-shadow/050.webp" },
	{ id: "shadow-053", url: "/screenshot-assets/overlay-shadow/053.webp" },
	{ id: "shadow-057", url: "/screenshot-assets/overlay-shadow/057.webp" },
	{ id: "shadow-063", url: "/screenshot-assets/overlay-shadow/063.webp" },
	{ id: "shadow-064", url: "/screenshot-assets/overlay-shadow/064.webp" },
	{ id: "shadow-082", url: "/screenshot-assets/overlay-shadow/082.webp" },
	{ id: "shadow-083", url: "/screenshot-assets/overlay-shadow/083.webp" },
	{ id: "shadow-088", url: "/screenshot-assets/overlay-shadow/088.webp" },
	{ id: "shadow-097", url: "/screenshot-assets/overlay-shadow/097.webp" },
	{ id: "shadow-099", url: "/screenshot-assets/overlay-shadow/099.webp" },
];

/** CSS `background` shorthand for a bundled image url. */
export function imageBackgroundCss(url: string): string {
	return `url("${url}") center / cover no-repeat`;
}
