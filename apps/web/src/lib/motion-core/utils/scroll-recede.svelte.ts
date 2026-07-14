import { prefersReducedMotion } from "./reduced-motion.svelte";

type Options = {
	/** Scale at rest (top of page). Kept > 1 so the image always overflows its
	 *  frame and its edges/corners are never exposed. */
	from?: number;
	/** Scale once the section has fully exited. Stays >= 1 for full cover. */
	to?: number;
	/** Progress (0..1 of the section scrolling out) at which the zoom starts. */
	start?: number;
};

/**
 * Scroll-driven zoom-out for a hero backdrop. The image starts gently zoomed in
 * and eases back toward its natural frame as the section scrolls away, so it
 * reads as pulling back into depth. Crucially it only ever scales BETWEEN values
 * >= 1, and never translates, so the photo stays fully covering and no corner is
 * ever revealed. rAF-throttled; cleared and inert under reduced motion.
 */
export function scrollRecede(node: HTMLElement, options: Options = {}) {
	// Kept close to 1 so we barely magnify past the image's native size: a large
	// zoom here upscales a `bg-cover` photo (WebP especially) and softens it. The
	// zoom-out is still perceptible on a full-bleed hero at this range.
	const from = options.from ?? 1.06;
	const to = options.to ?? 1.0;
	const start = options.start ?? 0.12;

	let ticking = false;
	let raf = 0;

	function apply() {
		ticking = false;
		if (prefersReducedMotion()) {
			node.style.transform = "";
			return;
		}
		const section = node.closest("section") ?? node.parentElement;
		if (!section) return;
		const rect = section.getBoundingClientRect();
		const height = rect.height || 1;
		// How far the section top has passed above the viewport top, 0..1.
		const exited = Math.min(Math.max(-rect.top / height, 0), 1);
		// Remap so nothing happens until `start`, then ease-in the zoom-out.
		const raw = Math.max(0, (exited - start) / (1 - start));
		const p = raw * raw;
		// Interpolate scale between two values that both keep full cover.
		const scale = from - (from - to) * p;
		node.style.transform = `scale(${scale})`;
	}

	function onScroll() {
		if (ticking) return;
		ticking = true;
		raf = requestAnimationFrame(apply);
	}

	apply();
	window.addEventListener("scroll", onScroll, { passive: true });
	window.addEventListener("resize", onScroll, { passive: true });
	return {
		destroy() {
			cancelAnimationFrame(raf);
			window.removeEventListener("scroll", onScroll);
			window.removeEventListener("resize", onScroll);
		},
	};
}
