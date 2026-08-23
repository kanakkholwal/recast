// Keyframe animation engine ported from the reference app: a preset is a set of
// tracks, each track a list of keyframes over time; playback interpolates the
// animatable properties at any moment and the stage applies them as a live CSS
// transform + opacity. The same interpolation drives the video export, so the
// clip matches the preview exactly.

export type Easing =
	| "linear"
	| "ease-in"
	| "ease-out"
	| "ease-in-out"
	| "ease-in-cubic"
	| "ease-out-cubic"
	| "ease-in-expo"
	| "ease-out-expo";

/** Everything an animation can move. Angles in deg, translate in percent of the
 * element, `perspective` in px, `opacity` 0..1. */
export interface AnimatableProperties {
	rotateX: number;
	rotateY: number;
	rotateZ: number;
	scale: number;
	translateX: number;
	translateY: number;
	perspective: number;
	opacity: number;
}

export const DEFAULT_PROPS: AnimatableProperties = {
	rotateX: 0,
	rotateY: 0,
	rotateZ: 0,
	scale: 1,
	translateX: 0,
	translateY: 0,
	perspective: 1600,
	opacity: 1,
};

export interface Keyframe {
	time: number; // ms
	props: Partial<AnimatableProperties>;
	easing: Easing;
}

/** A user-authored keyframe: like {@link Keyframe} but with a stable id and a
 * full property snapshot (captured from the live 3D transform). */
export interface KeyframeEntry {
	id: string;
	time: number;
	props: AnimatableProperties;
	easing: Easing;
}

/** Build a synthetic single-track preset from user keyframes, so the same
 * interpolation + playback + export path drives custom animations. Returns null
 * for fewer than one keyframe. */
export function keyframesToPreset(entries: KeyframeEntry[]): AnimationPreset | null {
	if (entries.length < 1) return null;
	const sorted = [...entries].sort((a, b) => a.time - b.time);
	const duration = Math.max(1, sorted[sorted.length - 1].time);
	return {
		id: "__keyframes__",
		name: "Custom",
		category: "reveal",
		duration,
		tracks: [
			{ keyframes: sorted.map((e) => ({ time: e.time, props: e.props, easing: e.easing })) },
		],
	};
}

export interface AnimationTrack {
	keyframes: Keyframe[];
}

export type AnimationCategory =
	| "reveal"
	| "slide"
	| "fade"
	| "flip"
	| "perspective"
	| "orbit"
	| "depth"
	| "kenburns";

export interface AnimationPreset {
	id: string;
	name: string;
	category: AnimationCategory;
	duration: number; // ms
	tracks: AnimationTrack[];
}

export const CATEGORY_LABELS: Record<AnimationCategory, string> = {
	reveal: "Reveal",
	slide: "Slide",
	fade: "Fade",
	flip: "Flip",
	perspective: "Perspective",
	orbit: "Orbit",
	depth: "Depth",
	kenburns: "Ken Burns",
};

// --- Easing -----------------------------------------------------------------

const EASINGS: Record<Easing, (t: number) => number> = {
	linear: (t) => t,
	"ease-in": (t) => t * t,
	"ease-out": (t) => 1 - (1 - t) * (1 - t),
	"ease-in-out": (t) => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2),
	"ease-in-cubic": (t) => t * t * t,
	"ease-out-cubic": (t) => 1 - Math.pow(1 - t, 3),
	"ease-in-expo": (t) => (t === 0 ? 0 : Math.pow(2, 10 * t - 10)),
	"ease-out-expo": (t) => (t === 1 ? 1 : 1 - Math.pow(2, -10 * t)),
};

function ease(progress: number, easing: Easing): number {
	const fn = EASINGS[easing] ?? EASINGS.linear;
	return fn(Math.max(0, Math.min(1, progress)));
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

// --- Interpolation ----------------------------------------------------------

function interpolateProperty(
	track: AnimationTrack,
	time: number,
	key: keyof AnimatableProperties,
	fallback: number,
): number {
	const sorted = [...track.keyframes].sort((a, b) => a.time - b.time);
	let prev: Keyframe | null = null;
	let next: Keyframe | null = null;
	for (const kf of sorted) {
		if (kf.time <= time) prev = kf;
		if (kf.time >= time && next === null) next = kf;
	}
	if (!prev && !next) return fallback;
	if (!prev && next) return next.props[key] ?? fallback;
	if (prev && !next) return prev.props[key] ?? fallback;
	if (prev && next) {
		if (prev === next) return prev.props[key] ?? fallback;
		const a = prev.props[key] ?? fallback;
		const b = next.props[key] ?? fallback;
		const span = next.time - prev.time;
		const raw = span > 0 ? (time - prev.time) / span : 1;
		return lerp(a, b, ease(raw, next.easing));
	}
	return fallback;
}

/** Interpolate every animatable property of a preset at `time` (ms). */
export function propsAtTime(preset: AnimationPreset, time: number): AnimatableProperties {
	const result: AnimatableProperties = { ...DEFAULT_PROPS };
	for (const track of preset.tracks) {
		const keys = new Set<keyof AnimatableProperties>();
		for (const kf of track.keyframes) {
			for (const k of Object.keys(kf.props) as (keyof AnimatableProperties)[]) keys.add(k);
		}
		for (const k of keys) {
			result[k] = interpolateProperty(track, time, k, DEFAULT_PROPS[k]);
		}
	}
	return result;
}

/** The CSS transform for a set of animated properties (perspective is applied
 * separately on the parent, so it's not included here). */
export function propsToTransform(p: AnimatableProperties): string {
	return `rotateX(${p.rotateX}deg) rotateY(${p.rotateY}deg) rotateZ(${p.rotateZ}deg) translate(${p.translateX}%, ${p.translateY}%) scale(${p.scale})`;
}

// --- Presets ----------------------------------------------------------------

const t = (
	name: string,
	category: AnimationCategory,
	duration: number,
	tracks: Keyframe[][],
): AnimationPreset => ({
	id: name.toLowerCase().replace(/[^a-z0-9]+/g, "-"),
	name,
	category,
	duration,
	tracks: tracks.map((keyframes) => ({ keyframes })),
});

const k = (
	time: number,
	props: Partial<AnimatableProperties>,
	easing: Easing = "ease-out",
): Keyframe => ({
	time,
	props,
	easing,
});

export const ANIMATION_PRESETS: AnimationPreset[] = [
	// Reveal
	t("Hero Landing", "reveal", 1200, [
		[
			k(0, { rotateX: 25, scale: 0.95, perspective: 2400 }),
			k(1200, { rotateX: 0, scale: 1, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(600, { opacity: 1 })],
	]),
	t("Slide In 3D", "reveal", 1000, [
		[
			k(0, { rotateY: 30, translateX: 35, perspective: 2400 }),
			k(1000, { rotateY: 0, translateX: 0, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(500, { opacity: 1 })],
	]),
	t("Rise & Settle", "reveal", 1000, [
		[
			k(0, { translateY: 25, rotateX: -15, perspective: 2400, scale: 0.97 }),
			k(1000, { translateY: 0, rotateX: 0, perspective: 2400, scale: 1 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(500, { opacity: 1 })],
	]),
	t("Drop In", "reveal", 1000, [
		[
			k(0, { translateY: -20, rotateX: 12, perspective: 2400, scale: 0.97 }),
			k(1000, { translateY: 0, rotateX: 0, perspective: 2400, scale: 1 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(500, { opacity: 1 })],
	]),
	// Slide
	t("Slide Up", "slide", 800, [
		[
			k(0, { translateY: 30, perspective: 2400 }),
			k(800, { translateY: 0, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(400, { opacity: 1 })],
	]),
	t("Slide Down", "slide", 800, [
		[
			k(0, { translateY: -30, perspective: 2400 }),
			k(800, { translateY: 0, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(400, { opacity: 1 })],
	]),
	t("Slide Left", "slide", 800, [
		[
			k(0, { translateX: 35, perspective: 2400 }),
			k(800, { translateX: 0, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(400, { opacity: 1 })],
	]),
	t("Slide Right", "slide", 800, [
		[
			k(0, { translateX: -35, perspective: 2400 }),
			k(800, { translateX: 0, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(400, { opacity: 1 })],
	]),
	// Fade
	t("Fade In", "fade", 800, [[k(0, { opacity: 0 }), k(800, { opacity: 1 })]]),
	t("Fade Scale", "fade", 800, [
		[
			k(0, { scale: 0.96, perspective: 2400 }),
			k(800, { scale: 1, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(600, { opacity: 1 })],
	]),
	t("Fade Rise", "fade", 800, [
		[
			k(0, { translateY: 12, scale: 0.98, perspective: 2400 }),
			k(800, { translateY: 0, scale: 1, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(600, { opacity: 1 })],
	]),
	t("Fade Zoom Out", "fade", 1000, [
		[
			k(0, { scale: 1.08, perspective: 2400 }),
			k(1000, { scale: 1, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(600, { opacity: 1 })],
	]),
	// Flip
	t("Flip X", "flip", 1500, [
		[
			k(0, { rotateX: 0, scale: 1, perspective: 2400 }),
			k(750, { rotateX: 90, scale: 0.95, perspective: 2400 }, "ease-in"),
			k(1500, { rotateX: 180, scale: 1, perspective: 2400 }),
		],
	]),
	t("Flip Y", "flip", 1500, [
		[
			k(0, { rotateY: 0, scale: 1, perspective: 2400 }),
			k(750, { rotateY: 90, scale: 0.95, perspective: 2400 }, "ease-in"),
			k(1500, { rotateY: 180, scale: 1, perspective: 2400 }),
		],
	]),
	t("Peek", "flip", 2000, [
		[
			k(0, { rotateY: 0, perspective: 2400 }),
			k(600, { rotateY: 35, perspective: 2400 }, "ease-out-cubic"),
			k(1400, { rotateY: 35, perspective: 2400 }, "ease-in-out"),
			k(2000, { rotateY: 0, perspective: 2400 }, "ease-out-cubic"),
		],
	]),
	t("Flip Reveal", "flip", 1200, [
		[
			k(0, { rotateY: -90, scale: 0.95, perspective: 2400 }),
			k(800, { rotateY: 5, scale: 1.02, perspective: 2400 }, "ease-out-cubic"),
			k(1200, { rotateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
		],
		[k(0, { opacity: 0 }), k(400, { opacity: 1 })],
	]),
	// Perspective
	t("Showcase Tilt", "perspective", 2500, [
		[
			k(0, { rotateY: 0, rotateX: 0, perspective: 2400 }, "ease-in-out"),
			k(2500, { rotateY: 18, rotateX: 6, perspective: 2400 }, "ease-in-out"),
		],
	]),
	t("Isometric", "perspective", 2000, [
		[
			k(0, { rotateX: 0, rotateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
			k(2000, { rotateX: 22, rotateY: -22, scale: 0.95, perspective: 2400 }, "ease-in-out"),
		],
	]),
	t("Hover Float", "perspective", 3000, [
		[
			k(0, { rotateX: 0, translateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
			k(750, { rotateX: 4, translateY: -3, scale: 1.01, perspective: 2400 }, "ease-in-out"),
			k(1500, { rotateX: 0, translateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
			k(2250, { rotateX: -4, translateY: 3, scale: 1.01, perspective: 2400 }, "ease-in-out"),
			k(3000, { rotateX: 0, translateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
		],
	]),
	t("Apple Showcase", "perspective", 1500, [
		[
			k(0, { rotateX: 20, rotateY: -15, scale: 0.96, perspective: 2400 }),
			k(1500, { rotateX: 5, rotateY: -8, scale: 1, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(600, { opacity: 1 })],
	]),
	// Orbit
	t("Orbit Left", "orbit", 2500, [
		[
			k(0, { rotateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
			k(1250, { rotateY: -25, scale: 0.97, perspective: 2400 }, "ease-in-out"),
			k(2500, { rotateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
		],
	]),
	t("Orbit Right", "orbit", 2500, [
		[
			k(0, { rotateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
			k(1250, { rotateY: 25, scale: 0.97, perspective: 2400 }, "ease-in-out"),
			k(2500, { rotateY: 0, scale: 1, perspective: 2400 }, "ease-in-out"),
		],
	]),
	t("Turntable", "orbit", 3000, [
		[
			k(0, { rotateY: 0, scale: 0.95, perspective: 2400 }, "linear"),
			k(3000, { rotateY: 360, scale: 0.95, perspective: 2400 }, "linear"),
		],
	]),
	t("Swing", "orbit", 2000, [
		[
			k(0, { rotateZ: 0, rotateY: 0, perspective: 2400 }, "ease-in-out"),
			k(500, { rotateZ: -8, rotateY: -10, perspective: 2400 }, "ease-in-out"),
			k(1000, { rotateZ: 0, rotateY: 0, perspective: 2400 }, "ease-in-out"),
			k(1500, { rotateZ: 6, rotateY: 8, perspective: 2400 }, "ease-in-out"),
			k(2000, { rotateZ: 0, rotateY: 0, perspective: 2400 }, "ease-in-out"),
		],
	]),
	// Depth
	t("Push Away", "depth", 2000, [
		[
			k(0, { scale: 1, perspective: 2400, rotateX: 0 }, "ease-in-out"),
			k(2000, { scale: 0.85, perspective: 1600, rotateX: 8 }, "ease-in-out"),
		],
	]),
	t("Pull Close", "depth", 1200, [
		[
			k(0, { scale: 0.95, perspective: 1800, rotateX: -4 }),
			k(1200, { scale: 1.03, perspective: 2400, rotateX: 0 }, "ease-out-cubic"),
		],
	]),
	t("Dramatic Zoom", "depth", 1200, [
		[
			k(0, { scale: 0.95, perspective: 1400 }),
			k(1200, { scale: 1.08, perspective: 2400 }, "ease-out-cubic"),
		],
		[k(0, { opacity: 0 }), k(500, { opacity: 1 })],
	]),
	t("Breathe 3D", "depth", 3000, [
		[
			k(0, { scale: 1, rotateX: 0, rotateY: 0, perspective: 2400 }, "ease-in-out"),
			k(1500, { scale: 1.03, rotateX: 2, rotateY: -2, perspective: 2400 }, "ease-in-out"),
			k(3000, { scale: 1, rotateX: 0, rotateY: 0, perspective: 2400 }, "ease-in-out"),
		],
	]),
	// Ken Burns
	t("Zoom In", "kenburns", 4000, [
		[
			k(0, { scale: 1, translateX: 0, translateY: 0, perspective: 2400 }, "ease-in-out"),
			k(4000, { scale: 1.15, translateX: 3, translateY: -2, perspective: 2400 }, "ease-in-out"),
		],
	]),
	t("Zoom Out", "kenburns", 4000, [
		[
			k(0, { scale: 1.12, translateX: -3, translateY: 2, perspective: 2400 }, "ease-in-out"),
			k(4000, { scale: 1, translateX: 0, translateY: 0, perspective: 2400 }, "ease-in-out"),
		],
	]),
	t("Pan Left", "kenburns", 4000, [
		[
			k(0, { translateX: 8, scale: 1.05, perspective: 2400 }, "ease-in-out"),
			k(4000, { translateX: -8, scale: 1.08, perspective: 2400 }, "ease-in-out"),
		],
	]),
	t("Pan Right", "kenburns", 4000, [
		[
			k(0, { translateX: -8, scale: 1.05, perspective: 2400 }, "ease-in-out"),
			k(4000, { translateX: 8, scale: 1.08, perspective: 2400 }, "ease-in-out"),
		],
	]),
];

export function presetById(id: string | null): AnimationPreset | null {
	if (!id) return null;
	return ANIMATION_PRESETS.find((p) => p.id === id) ?? null;
}

export function presetsByCategory(): {
	category: AnimationCategory;
	label: string;
	presets: AnimationPreset[];
}[] {
	const cats: AnimationCategory[] = [
		"reveal",
		"slide",
		"fade",
		"flip",
		"perspective",
		"orbit",
		"depth",
		"kenburns",
	];
	return cats.map((category) => ({
		category,
		label: CATEGORY_LABELS[category],
		presets: ANIMATION_PRESETS.filter((p) => p.category === category),
	}));
}
