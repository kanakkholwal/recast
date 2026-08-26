<script lang="ts">
import { cn } from "@recast/ui/utils";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

type Feature = {
	icon: any;
	tag: string;
	title: string;
	description: string;
	image: string | null;
};

let {
	features,
	class: className = "",
}: {
	features: Feature[];
	class?: string;
} = $props();

// Scroll-driven stepper.
//
// The outer section is `features.length × 100vh` tall. The inner content
// is `sticky top-0` and stays pinned for the whole scroll range. As the
// user scrolls, the active index tracks `window.scrollY` against the
// section's top edge:
//
//   - section top crossing viewport top  → first step
//   - one viewport per step             → each step owns a viewport
//   - section bottom crossing viewport bottom → last step
//
// Scrolling past the section releases the pin and the next section
// flows in normally; scrolling up reverses.
//
// Inside the sticky, the step list is a *sliding window* (3 steps
// visible at a time). As the active index advances, the inner list
// translates up so the active step is always vertically centered in
// the window and the previous/next steps peek above/below. The
// right panel morphs between icons with a slide-up transition so the
// visual matches the list motion.
let sectionEl: HTMLElement | undefined = $state();
let current = $state(0);
let progress = $state(0); // 0..1 across the whole section

// `visible` is how many steps show in the window. Three is the sweet
// spot the user asked for: one above, one active, one below. The
// window is `VISIBLE * stepH` tall, and each step is `stepH` tall, so
// the math falls out cleanly: `translateY(-current * stepH)` aligns
// step `current` with the center of the window.
const VISIBLE = 3;
const stepH = 80; // px per step inside the window (its visual height)

function recompute() {
	const el = sectionEl;
	if (!el) return;
	const rect = el.getBoundingClientRect();
	const sectionHeight = el.offsetHeight;
	const viewport = window.innerHeight;
	const scrollRange = sectionHeight - viewport;
	if (scrollRange <= 0) {
		progress = rect.top < 0 ? 1 : 0;
	} else {
		const scrolled = -rect.top;
		progress = Math.max(0, Math.min(1, scrolled / scrollRange));
	}
	const n = features.length;
	// Six features across six viewports: index = floor(progress * n).
	// Clamp so the first and last step "stick" for a beat at the
	// ends and don't pop early/late.
	const raw = Math.floor(progress * n);
	current = Math.max(0, Math.min(n - 1, raw));
}

$effect(() => {
	if (typeof window === "undefined") return;
	untrack(() => recompute());
	const onScroll = () => recompute();
	const onResize = () => recompute();
	window.addEventListener("scroll", onScroll, { passive: true });
	window.addEventListener("resize", onResize);
	return () => {
		window.removeEventListener("scroll", onScroll);
		window.removeEventListener("resize", onResize);
	};
});

// The step list translates by one step-height per index step. With
// VISIBLE=3 and stepH=80px, the active step sits in the middle of
// the window (offset = 1 * stepH from the top of the window).
const translateY = $derived(`translateY(-${current * stepH}px)`);
</script>

<section
	bind:this={sectionEl}
	class={cn("relative", className)}
	style="height: {features.length * 100}vh;"
>
	<!-- Sticky inner. Stays pinned for the entire scroll range, releases
	     when the user scrolls past the section's bottom edge. -->
	<div class="sticky top-0 grid h-screen place-items-center overflow-hidden">
		<div class="mx-auto w-full max-w-6xl px-6 sm:px-8 lg:px-10">
			<div class="grid items-center gap-12 lg:grid-cols-12 lg:gap-16">
				<!--
				  Left: sliding step window. Only VISIBLE (3) steps are shown
				  at a time. The list translates up by one step-height per
				  active-index step, so the active step is always vertically
				  centered in the window and the previous/next steps peek at
				  the top and bottom. Past steps are dimmed and stay visible
				  as context; future steps are dimmed at the bottom.
				-->
				<ol
					class="relative lg:col-span-7"
					style="height: {VISIBLE * stepH}px;"
				>
					<div
						class="absolute inset-0 flex flex-col gap-0 transition-transform duration-700 ease-out motion-reduce:transition-none"
						style="transform: {translateY};"
					>
						{#each features as feature, i (feature.title)}
							{@const Icon = feature.icon}
							{@const isActive = i === current}
							{@const isPast = i < current}
							<li
								style="height: {stepH}px;"
								class="flex items-center transition-[opacity,transform,color] duration-500 ease-out motion-reduce:transition-none"
								class:opacity-100={isActive}
								class:opacity-25={isPast}
								class:opacity-15={!isActive && !isPast}
							>
								<div
									class={cn(
										"flex w-full items-start gap-5 px-1 transition-transform duration-500",
										isActive && "translate-x-1",
									)}
								>
									<span
										class={cn(
											"mt-1 grid size-9 shrink-0 place-items-center rounded-full font-mono text-caption font-semibold tabular-nums transition-colors duration-500",
											isActive
												? "bg-foreground text-background"
												: "bg-foreground/[0.04] text-foreground/50",
										)}
									>
										{String(i + 1).padStart(2, "0")}
									</span>
									<div class="min-w-0 flex-1">
										<div class="flex items-center gap-2">
											<span
												class={cn(
													"inline-flex items-center rounded-full px-1.5 py-0.5 text-caption font-semibold transition-colors duration-500",
													isActive
														? "bg-paper text-foreground"
														: "bg-foreground/[0.03] text-muted-foreground",
												)}
											>
												{feature.tag}
											</span>
										</div>
										<h3
											class={cn(
												"mt-2 text-balance text-2xl font-semibold leading-[1.1] tracking-tight sm:text-3xl lg:text-[2rem]",
												isActive ? "text-foreground" : "text-foreground/65",
											)}
										>
											{feature.title}
										</h3>
										{#if isActive}
											<p
												class="mt-2 max-w-md text-pretty text-body leading-relaxed text-muted-foreground"
												in:fly={{ y: 6, duration: 300, easing: cubicOut }}
											>
												{feature.description}
											</p>
										{/if}
									</div>
								</div>
							</li>
						{/each}
					</div>
				</ol>

				<!--
				  Right: morphing icon panel. The previous icon slides up
				  and out, the next slides up and in. Real screenshot
				  when one exists, otherwise the same icon-as-hero
				  placeholder. The `tag` badge anchors bottom-right (matches
				  the vendor layout from the reference).
				-->
				<div class="relative hidden lg:col-span-5 lg:block">
					<div
						class="relative aspect-[4/3] overflow-hidden rounded-2xl border border-border-low bg-card/40"
					>
						{#each features as feature, i (feature.title)}
							{@const Icon = feature.icon}
							{#if i === current}
								<div
									class="absolute inset-0 flex flex-col items-center justify-center gap-6 p-8"
									in:fly={{ y: 40, duration: 500, easing: cubicOut }}
									out:fly={{ y: -40, duration: 300, easing: cubicOut }}
								>
									{#if feature.image}
										<img
											src={feature.image}
											alt={feature.title}
											loading="lazy"
											decoding="async"
											class="absolute inset-0 size-full object-cover"
										/>
									{:else}
										<div
											class="absolute inset-0"
											style="background: linear-gradient(160deg, color-mix(in srgb, var(--color-foreground) 7%, transparent) 0%, color-mix(in srgb, var(--color-foreground) 3%, transparent) 60%, transparent 100%);"
										></div>
										<div
											class="grid size-24 place-items-center rounded-2xl border border-border-low bg-card/60 shadow-craft-sm"
										>
											<Icon class="size-12 text-foreground" />
										</div>
										<div
											class="rounded-full bg-foreground/85 px-2 py-0.5 text-caption font-semibold text-background shadow-craft-sm"
										>
											{feature.tag}
										</div>
									{/if}
								</div>
							{/if}
						{/each}
					</div>
				</div>
			</div>
		</div>
	</div>
</section>