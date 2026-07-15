import { prefersReducedMotion } from "./reduced-motion.svelte";

/**
 * Svelte action for decorative autoplay loops (hero + proof clips).
 *
 * Two jobs the bare `autoplay loop` attributes can't do:
 *   - Pause when the clip scrolls out of view (battery, and it stops competing
 *     with the LCP element for decode bandwidth).
 *   - Honour reduced-motion: a looping video is exactly the kind of motion the
 *     preference asks to suppress, so we hold it on its poster/first frame.
 *
 * The element keeps its `autoplay` attribute as a no-JS fallback; this only
 * tightens the behaviour when JS is present.
 */
export function autoplayInView(node: HTMLVideoElement) {
	if (prefersReducedMotion()) {
		node.autoplay = false;
		node.pause();
		return {};
	}

	if (typeof IntersectionObserver === "undefined") return {};

	const io = new IntersectionObserver(
		([entry]) => {
			if (prefersReducedMotion()) {
				node.pause();
				return;
			}
			if (entry.isIntersecting) {
				// play() rejects if the tab can't autoplay; the poster stays, so
				// swallow it rather than throw an unhandled rejection.
				void node.play().catch(() => {});
			} else {
				node.pause();
			}
		},
		{ threshold: 0.25 },
	);
	io.observe(node);

	return {
		destroy() {
			io.disconnect();
		},
	};
}
