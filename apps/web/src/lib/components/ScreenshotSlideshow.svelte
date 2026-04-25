<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	type Slide = { src: string; alt: string };

	const slides: Slide[] = [
		{ src: "/screenshots/preview_homescreen.png", alt: "Recast home screen" },
		{ src: "/screenshots/preview_profiles.png", alt: "Recast export profiles" },
	];

	const INTERVAL_MS = 4000;
	const TRANSITION_MS = 900;

	// Build a long reel by repeating the slide list. We then advance a
	// monotonically-increasing `step` counter; when it nears the end of the
	// reel we silently snap back by `slides.length` slots so the same image
	// stays under the active position. That snap is invisible because the
	// slide N steps earlier shows the exact same picture.
	const REPEAT = Math.max(6, slides.length * 4);
	const reel: Slide[] = Array.from(
		{ length: REPEAT },
		(_, i) => slides[i % slides.length],
	);

	let step = $state(0);
	let snapping = $state(false);
	let timer: ReturnType<typeof setInterval> | null = null;

	function tick() {
		step += 1;
		// When step gets close to the reel boundary, schedule a silent reset
		// after the slide transition has finished.
		if (step >= reel.length - slides.length) {
			setTimeout(() => {
				snapping = true;
				step = step - slides.length;
				// Two RAFs: first commits the new step + `transition: none`,
				// second restores transitions so the next tick animates again.
				requestAnimationFrame(() => {
					requestAnimationFrame(() => {
						snapping = false;
					});
				});
			}, TRANSITION_MS);
		}
	}

	function start() {
		stop();
		timer = setInterval(tick, INTERVAL_MS);
	}

	function stop() {
		if (timer) {
			clearInterval(timer);
			timer = null;
		}
	}

	onMount(start);
	onDestroy(stop);
</script>

<div
	class="slideshow"
	class:snapping
	role="region"
	aria-roledescription="carousel"
	aria-label="Product screenshots"
	onmouseenter={stop}
	onmouseleave={start}
	style="--step: {step}; --transition-ms: {TRANSITION_MS}ms;"
>
	<div class="track">
		{#each reel as slide, i (i)}
			<div
				class="slide"
				class:active={i === step}
				style="--i: {i};"
				aria-hidden={i !== step}
			>
				<img
					src={slide.src}
					alt={slide.alt}
					loading={i === 0 ? "eager" : "lazy"}
					draggable="false"
				/>
			</div>
		{/each}
	</div>
</div>

<style>
	.slideshow {
		--radius: 1.5rem;
		--gap: 2.5rem;
		--slide-width: min(56rem, 100%);
		--inactive-scale: 0.82;
		--inactive-opacity: 0.35;
		position: relative;
		width: 100%;
		max-width: 80rem;
		margin: 0 auto;
		overflow: hidden;
		padding: 2rem 0;
		mask-image: linear-gradient(
			to right,
			transparent 0%,
			#000 12%,
			#000 88%,
			transparent 100%
		);
		-webkit-mask-image: linear-gradient(
			to right,
			transparent 0%,
			#000 12%,
			#000 88%,
			transparent 100%
		);
	}

	.track {
		position: relative;
		height: clamp(18rem, 38vw, 32rem);
		width: 100%;
	}

	.slide {
		position: absolute;
		top: 0;
		left: 50%;
		width: var(--slide-width);
		height: 100%;
		/* Slot offset: every slide stays at a fixed position relative to the
		   advancing `--step` counter. Active slide (i === step) lands at 0,
		   the next sits one slot to the right, the previous one slot left. */
		transform: translate(-50%, 0)
			translateX(
				calc((var(--i) - var(--step)) * (var(--slide-width) + var(--gap)))
			)
			scale(var(--inactive-scale));
		opacity: var(--inactive-opacity);
		filter: blur(2px);
		transition:
			transform var(--transition-ms) cubic-bezier(0.22, 1, 0.36, 1),
			opacity calc(var(--transition-ms) - 200ms) ease,
			filter calc(var(--transition-ms) - 200ms) ease;
		will-change: transform, opacity, filter;
		pointer-events: none;
	}

	.slide.active {
		transform: translate(-50%, 0) translateX(0) scale(1);
		opacity: 1;
		filter: blur(0);
		pointer-events: auto;
	}

	/* During the silent snap-back, suppress transitions so step can jump
	   without animating across the whole reel. */
	.slideshow.snapping .slide {
		transition: none;
	}

	.slide img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		border-radius: var(--radius);
		background: hsl(var(--muted) / 0.4);
		box-shadow:
			0 1px 0 0 hsl(var(--border) / 0.6),
			0 24px 60px -20px rgb(0 0 0 / 0.35),
			0 8px 24px -8px rgb(0 0 0 / 0.18);
		user-select: none;
		-webkit-user-drag: none;
	}

	@media (prefers-reduced-motion: reduce) {
		.slide {
			transition: opacity 200ms ease;
			filter: none;
		}
	}
</style>
