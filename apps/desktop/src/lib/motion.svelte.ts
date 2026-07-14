// Reactive `prefers-reduced-motion`, readable anywhere: modules, `$derived`,
// or inline transition params.
//
// Svelte 5 runs its built-in transitions (fade/fly/slide/...) through
// `element.animate()` (WAAPI). The global `@media (prefers-reduced-motion)`
// override in app.css only zeroes CSS `transition-duration`/`animation-duration`,
// which WAAPI ignores, so those transitions keep their full duration under
// "reduce motion". Durations that feed a Svelte transition or a JS-driven
// animation must therefore be zeroed in JS, via `motionDuration()` below.

let reduced = $state(false);

// Initialised eagerly at module load, NOT lazily inside the getter: the getter
// is read from inside `$derived`s (e.g. the playhead tween), and writing `$state`
// during a derived is forbidden (state_unsafe_mutation). Module init runs outside
// any reactive context, so seeding + subscribing here is safe. Guarded for
// SSR/prerender, where `window` is absent and `reduced` simply stays false.
if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
  const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
  reduced = mq.matches;
  mq.addEventListener("change", (e) => (reduced = e.matches));
}

/** True when the OS is set to reduce motion. Reactive: re-reads when toggled. */
export function prefersReducedMotion(): boolean {
  return reduced;
}

/** A duration (ms) that collapses to 0 when the user asked for less motion. */
export function motionDuration(ms: number): number {
  return reduced ? 0 : ms;
}
