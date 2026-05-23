<script lang="ts">
	import { navigating } from "$app/state";
	import { cubicOut } from "svelte/easing";
	import { Tween } from "svelte/motion";

	/**
	 * Top-of-page navigation progress bar — same UX as Vercel/Linear.
	 *
	 * Driven by SvelteKit's `navigating` store (truthy whenever a `goto()` /
	 * link click is mid-flight). On start: jumps to `minimum`, then trickles
	 * up toward 0.9 while the load is pending — never reaches 1.0 until the
	 * navigation actually completes. On finish: snaps to 1.0, fades out.
	 *
	 * `color` defaults to the design system's `--color-primary` so the bar
	 * inherits theme accents without per-app overrides.
	 */

	let {
		color = "var(--color-primary)",
		height = 3,
		trickleSpeed = 200,
		minimum = 0.08,
		duration = 300,
		shadow = true,
	}: {
		color?: string;
		height?: number;
		trickleSpeed?: number;
		minimum?: number;
		duration?: number;
		shadow?: boolean;
	} = $props();

	const progress = new Tween(0, { duration: () => duration, easing: cubicOut });

	let visible = $state(false);
	let trickleInterval: ReturnType<typeof setInterval> | null = null;
	// Bumped on every effect run. Deferred completion work (the `.then` and
	// `setTimeout` below) captures the value at schedule time and bails if a
	// newer navigation has started — otherwise stale callbacks can hide the
	// bar mid-way through a chained navigation.
	let navGeneration = 0;

	function startTrickle() {
		if (trickleInterval) return;
		trickleInterval = setInterval(() => {
			const remaining = 0.9 - progress.target;
			if (remaining <= 0) return;
			const increment = remaining * 0.1 + Math.random() * 0.02;
			progress.set(Math.min(progress.target + increment, 0.9));
		}, trickleSpeed);
	}

	function stopTrickle() {
		if (trickleInterval) {
			clearInterval(trickleInterval);
			trickleInterval = null;
		}
	}

	$effect(() => {
		const active = navigating.to !== null;
		const token = ++navGeneration;
		if (active) {
			visible = true;
			progress.set(minimum, { duration: 0 });
			startTrickle();
		} else {
			stopTrickle();
			progress.set(1, { duration: duration * 0.5 }).then(() => {
				// Bail if a newer navigation started between schedule and resolve.
				if (token !== navGeneration) return;
				setTimeout(() => {
					if (token !== navGeneration) return;
					visible = false;
					progress.set(0, { duration: 0 });
				}, 200);
			});
		}
	});
</script>

{#if visible}
	<div
		class="nav-progress"
		style="
			--progress: {progress.current};
			--color: {color};
			--height: {height}px;
			--shadow: {shadow ? `0 0 8px ${color}99, 0 0 2px ${color}` : 'none'};
		"
		aria-hidden="true"
	>
		<div class="bar"></div>
	</div>
{/if}

<style>
	.nav-progress {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 9999;
		pointer-events: none;
		height: var(--height);
	}

	.bar {
		height: 100%;
		background: var(--color);
		box-shadow: var(--shadow);
		border-radius: 0 2px 2px 0;
		transform-origin: left center;
		transform: scaleX(var(--progress));
		transition: transform 0ms; /* tweened handles animation */
	}
</style>
