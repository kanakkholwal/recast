// Svelte's transitions run through WAAPI, which the CSS reduced-motion override never reaches, so JS-fed durations must be zeroed via `motionDuration()`. Backed by the framework's own query so `@recast/ui` agrees.

import { prefersReducedMotion as reducedMotion } from "svelte/motion";

/** True when the OS is set to reduce motion. Reactive: re-reads when toggled. */
export function prefersReducedMotion(): boolean {
	return reducedMotion.current;
}

/** A duration (ms) that collapses to 0 when the user asked for less motion. */
export function motionDuration(ms: number): number {
	return reducedMotion.current ? 0 : ms;
}
