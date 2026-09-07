import type { AnimationConfig } from "svelte/animate";
import { cubicOut } from "svelte/easing";

interface MorphParams {
	duration?: number;
	easing?: (t: number) => number;
}

/**
 * FLIP-with-scale layout animation. Unlike Svelte's built-in `flip` (translate
 * only) this also tweens scale, so a keyed element changing position *and* size
 * morphs between shapes (cf. Framer Motion's `layout`).
 *
 * Apply with `animate:morph` on a keyed `{#each}` element; the each block must
 * re-run for the animation to fire.
 */
export function morph(
	_node: Element,
	{ from, to }: { from: DOMRect; to: DOMRect },
	params: MorphParams = {},
): AnimationConfig {
	const dx = from.left - to.left;
	const dy = from.top - to.top;
	const dw = to.width === 0 ? 1 : from.width / to.width;
	const dh = to.height === 0 ? 1 : from.height / to.height;

	const reduced =
		typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

	return {
		duration: reduced ? 0 : (params.duration ?? 320),
		easing: params.easing ?? cubicOut,
		// `u` is 1 - t: at t=0 the element paints at its previous rect, then settles into the new one.
		css: (t, u) =>
			`transform-origin: top left; transform: translate(${u * dx}px, ${u * dy}px) scale(${t + u * dw}, ${t + u * dh});`,
	};
}
