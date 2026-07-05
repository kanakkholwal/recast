export type Slide = { src: string; alt: string };

export const INTERVAL_MS = 4000;
export const TRANSITION_MS = 900;

export const slides: Slide[] = [
	{ src: "/screenshots/preview_homescreen.png", alt: "Recast home screen" },
	{ src: "/screenshots/preview_profiles.png", alt: "Recast export profiles" },
];

// Build a long reel by repeating the slide list. The component advances a
// monotonically-increasing `step` counter; when it nears the end of the
// reel it silently snaps back by `slides.length` slots so the same image
// stays under the active position. That snap is invisible because the slide
// N steps earlier shows the exact same picture.
export function buildReel(source: Slide[]): Slide[] {
	const repeat = Math.max(6, source.length * 4);
	return Array.from({ length: repeat }, (_, i) => source[i % source.length]);
}
