import { cubicOut } from "svelte/easing";
import type { AnimationConfig } from "svelte/animate";

interface MorphParams {
  duration?: number;
  easing?: (t: number) => number;
}

/**
 * FLIP-with-scale layout animation.
 *
 * Svelte's built-in `flip` only translates; this also tweens scale, so a
 * keyed element whose position *and* size change between renders (e.g. a
 * card moving from a grid cell to a list row) visibly morphs between the
 * two shapes — the same effect as Framer Motion's `layout` animation.
 *
 * Apply with `animate:morph` on the top-level element of a keyed `{#each}`.
 * The each block must re-run for the animation to fire (toggle a layout
 * flag the iterated value depends on).
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
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  return {
    duration: reduced ? 0 : (params.duration ?? 320),
    easing: params.easing ?? cubicOut,
    // `u` = 1 - t: at t=0 the element is painted at its previous (`from`)
    // rect, then settles into the new (`to`) rect.
    css: (t, u) =>
      `transform-origin: top left; transform: translate(${u * dx}px, ${u * dy}px) scale(${t + u * dw}, ${t + u * dh});`,
  };
}
