// Svelte transitions run through WAAPI, which the CSS reduced-motion override can't reach, so gate on this in JS; init is eager so reads stay side-effect-free.
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
