// Shared reduced-motion signal for the web app.
//
// Svelte's `transition:`/`in:`/`out:` directives run through the Web Animations
// API, which the global `@media (prefers-reduced-motion: reduce)` override in
// app.css cannot reach. Any mount/scroll transition that should honour the
// user's motion preference has to gate on this in JS. Init is eager at module
// load (not lazy inside the getter) so reads stay side-effect-free and safe to
// call from a `$derived`.
let reduced = $state(false);

if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
	const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
	reduced = mq.matches;
	mq.addEventListener("change", (e) => (reduced = e.matches));
}

/** Reactive: true when the user has asked the OS to reduce motion. */
export function prefersReducedMotion() {
	return reduced;
}
