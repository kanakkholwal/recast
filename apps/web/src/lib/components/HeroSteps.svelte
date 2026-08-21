<script lang="ts">
import { prefersReducedMotion } from "$lib/motion-core";
import { buttonVariants } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import type { HeroStep } from "./Hero.logic";

let {
	steps,
	fallbackSrc = "",
	poster = "/product_preview_hero.webp",
	class: className = "",
}: {
	steps: HeroStep[];
	fallbackSrc?: string;
	poster?: string;
	class?: string;
} = $props();

const reduced = $derived(prefersReducedMotion());

let active = $state(0);
let paused = $state(false);
let tabEls = $state<Array<HTMLButtonElement | null>>([]);
let videoEls = $state<Array<HTMLVideoElement | null>>([]);

const DWELL_MS = 5200;

// Auto-advance. Reduced motion pins the shelf to whatever the visitor picked,
// so the steps never move under someone who asked for stillness.
$effect(() => {
	if (reduced || paused) return;
	const id = setInterval(() => {
		active = (active + 1) % steps.length;
	}, DWELL_MS);
	return () => clearInterval(id);
});

// Only the visible clip decodes. Three simultaneous <video> decodes is the
// cheapest way to make a landing page stutter on a laptop.
$effect(() => {
	const current = active;
	videoEls.forEach((el, i) => {
		if (!el) return;
		if (i === current) {
			el.currentTime = 0;
			void el.play().catch(() => {});
		} else {
			el.pause();
		}
	});
});

function select(i: number) {
	active = i;
	tabEls[i]?.focus();
}

function onKeydown(e: KeyboardEvent) {
	const last = steps.length - 1;
	if (e.key === "ArrowRight") select(active === last ? 0 : active + 1);
	else if (e.key === "ArrowLeft") select(active === 0 ? last : active - 1);
	else if (e.key === "Home") select(0);
	else if (e.key === "End") select(last);
	else return;
	e.preventDefault();
}

// One accent per tag, never two on one component.
const accentClass = {
	tangerine: "text-tag-tangerine",
	lavender: "text-tag-lavender",
	green: "text-tag-green",
} as const;
</script>

<section
	role="group"
	aria-label="How Recast works"
	class={cn("relative bg-paper", className)}
	onpointerenter={() => (paused = true)}
	onpointerleave={() => (paused = false)}
	onfocusin={() => (paused = true)}
	onfocusout={() => (paused = false)}
>
	
	<div
		class="shelf relative z-10 mx-auto w-[calc(100%-2.5rem)] rounded-b-[20px] bg-background px-2 pb-4 sm:w-fit sm:rounded-b-[40px] sm:px-12 sm:pb-5 lg:px-24"
	>
		<div
			role="tablist"
			tabindex="-1"
			aria-label="Recast workflow steps"
			class="flex items-center justify-center gap-1"
			onkeydown={onKeydown}
		>
			{#each steps as step, i (step.id)}
				{@const Icon = step.icon}
				{@const on = active === i}
				<button
					bind:this={tabEls[i]}
					type="button"
					role="tab"
					id={`hero-tab-${step.id}`}
					aria-selected={on}
					aria-controls={`hero-panel-${step.id}`}
					tabindex={on ? 0 : -1}
					onclick={() => (active = i)}
					class={cn(
						buttonVariants({ variant: "ghost", size: "default" }),
						"relative isolate border-transparent bg-transparent hover:bg-transparent",
						// Button padding from `sm` up; tightened below it so three tabs plus
						// both fillets still fit a 320px viewport without a scroller.
						"gap-1.5 px-2 text-xs sm:gap-2 sm:px-5 sm:text-sm",
						on ? "text-foreground" : "text-muted-foreground hover:text-foreground",
					)}
				>
					<!-- Selected skin cross-fades in underneath the label instead of the
					     background tweening through an in-between grey. Unselected is bare:
					     on a white shelf a resting fill only adds noise. -->
					<span
						aria-hidden="true"
						class="absolute inset-0 -z-10 rounded-lg bg-card shadow-craft-sm transition-opacity duration-300 motion-reduce:transition-none"
						style={`opacity:${on ? 1 : 0}`}
					></span>
					<Icon
						class={cn("size-4 shrink-0 [fill-opacity:0.2] sm:size-4.5", accentClass[step.accent])}
						fill="currentColor"
					/>
					{step.label}
				</button>
			{/each}
		</div>
	</div>

	<div class="mx-auto w-full max-w-6xl px-6 pt-8 pb-12 sm:px-8 lg:px-10">
		<div class="mockup-frame overflow-hidden p-2 sm:p-2.5">
			<div class="relative aspect-video w-full overflow-hidden rounded-xl border border-border-low bg-paper">
				{#each steps as step, i (step.id)}
					{@const on = active === i}
					{@const src = step.src || fallbackSrc}
					<div
						id={`hero-panel-${step.id}`}
						role="tabpanel"
						aria-labelledby={`hero-tab-${step.id}`}
						aria-hidden={!on}
						class="absolute inset-0 transition-opacity ease-out motion-reduce:transition-none"
						style={`opacity:${on ? 1 : 0}; transition-duration:${reduced ? 0 : 420}ms;`}
					>
						{#if src}
							<!-- svelte-ignore a11y_media_has_caption -->
							<video
								bind:this={videoEls[i]}
								{src}
								{poster}
								loop
								muted
								playsinline
								preload={i === 0 ? "metadata" : "none"}
								class="block size-full object-cover"
							></video>
						{:else}
							<img
								src={poster}
								alt={`Recast — ${step.label}`}
								width="1920"
								height="1080"
								loading={i === 0 ? "eager" : "lazy"}
								decoding="async"
								class="block size-full object-cover"
							/>
						{/if}
					</div>
				{/each}
			</div>
		</div>

		<p class="mt-4 text-center text-body-sm text-muted-foreground">
			{steps[active]?.caption}
		</p>
	</div>
</section>

<style>
	/* Concave fillets joining the shelf back to the hero's bottom edge, drawn as
	   pseudo-elements of the shelf so they are flush by construction.

	   Each is a box sitting beside the shelf, painted with a radial gradient whose
	   circle is centred on the box's OUTER bottom corner: inside that circle is the
	   band showing through, outside it is hero white. That is the fillet.

	   Deliberately literal values rather than a --notch custom property: a var()
	   inside a gradient's radius is the one part of this that silently degrades to
	   an unpainted box. Box is 1px wider than its offset so it laps the shelf and
	   the shared vertical edge carries no antialiasing seam. */
	.shelf::before,
	.shelf::after {
		content: "";
		position: absolute;
		top: 0;
		z-index: -1;
		width: 21px;
		height: 20px;
		pointer-events: none;
	}
	.shelf::before {
		left: -20px;
		background-image: radial-gradient(circle 20px at 0 20px, transparent 19.5px, var(--color-background) 20px);
	}
	.shelf::after {
		right: -20px;
		background-image: radial-gradient(circle 20px at 21px 20px, transparent 19.5px, var(--color-background) 20px);
	}

	@media (min-width: 40rem) {
		.shelf::before,
		.shelf::after {
			width: 41px;
			height: 40px;
		}
		.shelf::before {
			left: -40px;
			background-image: radial-gradient(circle 40px at 0 40px, transparent 39.5px, var(--color-background) 40px);
		}
		.shelf::after {
			right: -40px;
			background-image: radial-gradient(circle 40px at 41px 40px, transparent 39.5px, var(--color-background) 40px);
		}
	}
</style>
