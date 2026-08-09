// Reactive `prefers-reduced-motion`, readable anywhere: modules, `$derived`,
// or inline transition params.
//
// Svelte 5 runs its built-in transitions (fade/fly/slide/...) through
// `element.animate()` (WAAPI). The global `@media (prefers-reduced-motion)`
// override in app.css only zeroes CSS `transition-duration`/`animation-duration`,
// which WAAPI ignores, so those transitions keep their full duration under
// "reduce motion". Durations that feed a Svelte transition or a JS-driven
// animation must therefore be zeroed in JS, via `motionDuration()` below.
//
// Backed by the framework's own media query rather than a second listener, so
// packages that can't import this module (`@recast/ui`) can read
// `prefersReducedMotion.current` from `svelte/motion` and always agree with it.

import { prefersReducedMotion as reducedMotion } from "svelte/motion";

/** True when the OS is set to reduce motion. Reactive: re-reads when toggled. */
export function prefersReducedMotion(): boolean {
	return reducedMotion.current;
}

/** A duration (ms) that collapses to 0 when the user asked for less motion. */
export function motionDuration(ms: number): number {
	return reducedMotion.current ? 0 : ms;
}
