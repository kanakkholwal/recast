<script lang="ts" module>
import type { IconComponent } from "@recast/icons";

export type MarqueeFeature = {
	kind: "auto" | "manual";
	icon: IconComponent;
	title: string;
	description: string;
};
</script>

<script lang="ts">
import { cn } from "@recast/ui/utils";

// The editor tour as one slow horizontal loop rather than a rail the visitor
// has to drag. The track holds the list twice and translates exactly -50%, so
// the seam lands on an identical frame and the loop is invisible.
//
// The second copy is aria-hidden: it is the same content, and a screen reader
// should hear the tour once.
//
// Card art is vector only. Screenshots in a 64s loop are a lot of bytes for
// something that scrolls past, and they age with every UI change; a duotone
// glyph over an animated ring stays on-system and weighs nothing.
let {
	items,
	/** Seconds for one full pass. Slow on purpose — this is ambient, not a carousel. */
	duration = 64,
	class: className = "",
}: {
	items: MarqueeFeature[];
	duration?: number;
	class?: string;
} = $props();

const badgeLabel = { auto: "Auto", manual: "Manual" } as const;
</script>

<div
	class={cn("marquee group relative overflow-hidden", className)}
	style={`--marquee-duration:${duration}s`}
>
	<div class="track flex w-max gap-4">
		{#each [0, 1] as copy (copy)}
			<div class="flex shrink-0 gap-4" aria-hidden={copy === 1 ? "true" : undefined}>
				{#each items as item, i (item.title)}
					{@const Icon = item.icon}
					<article class="w-72 shrink-0 sm:w-80">
						<div
							class="art relative grid h-40 place-items-center overflow-hidden rounded-xl border border-border-low bg-paper"
							style={`--art-delay:${i * 500}ms`}
						>
							<!-- Two rings ping outward under the glyph. Decorative, so the
							     svg is hidden from the accessibility tree. -->
							<svg
								viewBox="0 0 120 120"
								aria-hidden="true"
								class="absolute inset-0 size-full text-border-strong"
							>
								<circle class="ping" cx="60" cy="60" r="22" fill="none" stroke="currentColor" stroke-width="1" />
								<circle
									class="ping ping-2"
									cx="60"
									cy="60"
									r="22"
									fill="none"
									stroke="currentColor"
									stroke-width="1"
								/>
							</svg>

							<Icon
								class={cn(
									"breathe relative size-10 [fill-opacity:0.15]",
									item.kind === "auto" ? "text-tag-green" : "text-muted-foreground",
								)}
								fill="currentColor"
							/>

							<span
								class="absolute bottom-3 left-3 inline-flex items-center gap-1.5 rounded-full border border-border-low bg-card px-2 py-0.5 text-caption font-medium text-muted-foreground"
							>
								<span
									class={cn(
										"size-1 rounded-full",
										item.kind === "auto" ? "bg-tag-green" : "bg-border-strong",
									)}
								></span>
								{badgeLabel[item.kind]}
							</span>
						</div>

						<h3 class="mt-4 font-display text-body font-medium text-foreground">{item.title}</h3>
						<p class="mt-1 text-body-sm text-muted-foreground">{item.description}</p>
					</article>
				{/each}
			</div>
		{/each}
	</div>
</div>

<style>
	/* Cross-fade at both edges so cards dissolve into the page instead of being
	   sliced by the container. */
	.marquee {
		-webkit-mask-image: linear-gradient(to right, transparent, #000 6%, #000 94%, transparent);
		mask-image: linear-gradient(to right, transparent, #000 6%, #000 94%, transparent);
	}

	.track {
		animation: marquee var(--marquee-duration) linear infinite;
	}

	/* Hovering parks the loop so a card can actually be read. */
	.marquee:hover .track,
	.marquee:focus-within .track {
		animation-play-state: paused;
	}

	@keyframes marquee {
		to {
			transform: translateX(calc(-50% - 0.5rem));
		}
	}

	.breathe {
		animation: breathe 5s ease-in-out infinite;
		animation-delay: var(--art-delay, 0ms);
	}

	@keyframes breathe {
		0%,
		100% {
			opacity: 0.75;
			transform: scale(1);
		}
		50% {
			opacity: 1;
			transform: scale(1.06);
		}
	}

	.ping {
		transform-origin: center;
		animation: ping 5s ease-out infinite;
		animation-delay: var(--art-delay, 0ms);
		opacity: 0;
	}

	.ping-2 {
		animation-delay: calc(var(--art-delay, 0ms) + 1.6s);
	}

	@keyframes ping {
		0% {
			transform: scale(0.8);
			opacity: 0;
		}
		25% {
			opacity: 0.5;
		}
		100% {
			transform: scale(2.1);
			opacity: 0;
		}
	}

	/* The global reduced-motion guard collapses animation-duration to 0.01ms,
	   which would snap the track to its end frame. Kill every loop outright and
	   let the rail be an ordinary scroller instead. */
	@media (prefers-reduced-motion: reduce) {
		.track,
		.breathe,
		.ping {
			animation: none;
		}
		.breathe {
			opacity: 1;
		}
		.ping {
			opacity: 0.35;
		}
		.marquee {
			overflow-x: auto;
		}
	}
</style>
