<script lang="ts">
import { prefersReducedMotion } from "$lib/motion-core";
import { cn } from "@recast/ui/utils";
import type { HeroStep } from "./Hero.logic";

// Record → Polish → Share as a tab shelf that hangs down out of the white
// hero into the paper band, joined by two concave fillets. One clip per step;
// until each step has its own take they all fall back to `fallbackSrc`, so the
// cross-fade is already wired when the assets land.
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
	tangerine: "bg-tag-tangerine/12 text-tag-tangerine",
	lavender: "bg-tag-lavender/12 text-tag-lavender",
	green: "bg-tag-green/12 text-tag-green",
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
	<!-- Shelf. Same white as the hero canvas above, so it reads as the hero
	     bulging downward rather than as a bar dropped onto the band. -->
	<div class="shelf relative z-10 mx-auto w-fit rounded-b-[28px] bg-background px-3 pb-3">
		<span aria-hidden="true" class="notch notch-l"></span>
		<span aria-hidden="true" class="notch notch-r"></span>

		<div
			role="tablist"
			tabindex="-1"
			aria-label="Recast workflow steps"
			class="flex items-center gap-1"
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
						"inline-flex items-center gap-2 rounded-lg px-3.5 py-2.5 text-body-sm font-medium transition-colors duration-200",
						"focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none",
						on
							? "border border-border-low bg-card text-foreground shadow-craft-sm"
							: "border border-transparent text-muted-foreground hover:text-foreground",
					)}
				>
					<span class={cn("grid size-5 shrink-0 place-items-center rounded-md", accentClass[step.accent])}>
						<Icon class="size-3.5" />
					</span>
					{step.label}
				</button>
			{/each}
		</div>
	</div>

	<div class="mx-auto w-full max-w-6xl px-6 pt-10 pb-16 sm:px-8 lg:px-10">
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

		<p class="mt-5 text-center text-body-sm text-muted-foreground">
			{steps[active]?.caption}
		</p>
	</div>
</section>

<style>
	/* Concave fillets joining the shelf back to the hero's bottom edge. The mask
	   keeps the outer corner opaque and removes the quarter-disc nearest the
	   shelf, which is what turns a butt joint into a notch. Radius is a shape
	   constant, not a component radius, so it sits outside the 8/12/16 set. */
	.notch {
		position: absolute;
		top: 0;
		width: 40px;
		height: 40px;
		background-color: var(--color-background);
	}
	.notch-l {
		left: -40px;
		-webkit-mask-image: radial-gradient(circle 40px at 0 100%, transparent 39.5px, #000 40px);
		mask-image: radial-gradient(circle 40px at 0 100%, transparent 39.5px, #000 40px);
	}
	.notch-r {
		right: -40px;
		-webkit-mask-image: radial-gradient(circle 40px at 100% 100%, transparent 39.5px, #000 40px);
		mask-image: radial-gradient(circle 40px at 100% 100%, transparent 39.5px, #000 40px);
	}
</style>
