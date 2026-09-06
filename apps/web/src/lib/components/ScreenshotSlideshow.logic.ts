export type Slide = { src: string; alt: string };

export const INTERVAL_MS = 4000;
export const TRANSITION_MS = 900;

export const slides: Slide[] = [
	{ src: "/screenshots/preview_homescreen.png", alt: "Recast home screen" },
	{ src: "/screenshots/preview_profiles.png", alt: "Recast export profiles" },
];

// A repeated reel: the component advances a monotonic step and snaps back by one list length, invisible because that slide shows the same picture.
export function buildReel(source: Slide[]): Slide[] {
	const repeat = Math.max(6, source.length * 4);
	return Array.from({ length: repeat }, (_, i) => source[i % source.length]);
}
