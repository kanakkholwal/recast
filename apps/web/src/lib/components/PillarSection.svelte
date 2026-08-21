<script lang="ts">
import type { IconComponent } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Reveal } from "@recast/ui/reveal";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import Container from "./Container.svelte";
import type { FeatureColumn } from "./FeatureColumns.svelte";
import FeatureColumns from "./FeatureColumns.svelte";
import SectionLabel from "./SectionLabel.svelte";

// The landing page's product-pillar template: a left-aligned label, heading,
// short description and one outlined action, then a full-bleed tonal band
// holding the product visual, closed by a three-up detail row.
//
// Left-aligned on purpose. A centred stack reads as a slide; this reads as a
// page, and it leaves the band below the full column width.
//
// Scroll-in comes from <Reveal>, not a local IntersectionObserver: Reveal
// already falls back to visible where the observer is missing, resets itself
// under prefers-reduced-motion, and carries the one shared easing curve. A
// hand-rolled copy leaves the section stuck at opacity-0 when JS never runs.
let {
	id,
	icon,
	label,
	accent = "primary",
	title,
	description,
	ctaHref,
	ctaLabel,
	features,
	visual,
	class: className = "",
}: {
	id?: string;
	icon?: IconComponent;
	label: string;
	accent?: "tangerine" | "lavender" | "green" | "primary";
	title: string;
	description: string;
	ctaHref?: string;
	ctaLabel?: string;
	features: FeatureColumn[];
	visual: Snippet;
	class?: string;
} = $props();
</script>

<section {id} class={cn("relative", className)}>
	<Container class="py-20 md:py-24">
		<!-- max-w-lg, not max-w-xl: the description should break to two lines, so
		     the block stays a paragraph rather than a banner. -->
		<div class="max-w-lg">
			<Reveal variant="up">
				<SectionLabel {icon} {label} {accent} />
			</Reveal>

			<Reveal variant="up" delay={60} class="mt-5">
				<h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
					{title}
				</h2>
			</Reveal>

			<Reveal variant="up" delay={120} class="mt-4">
				<p class="text-pretty text-body-lg text-muted-foreground">
					{description}
				</p>
			</Reveal>

			{#if ctaHref && ctaLabel}
				<Reveal variant="up" delay={180} class="mt-8">
					<Button href={ctaHref} variant="outline">{ctaLabel}</Button>
				</Reveal>
			{/if}
		</div>
	</Container>

	<!-- Tonal bands run full-bleed; borderless sections stay inside the column
	     rules. That alternation is what gives the page its rhythm. -->
	<div class="border-y border-border-low bg-paper">
		<Container class="py-12 md:py-16">
			<Reveal variant="up" delay={80}>
				{@render visual()}
			</Reveal>
		</Container>

		<Container>
			<Reveal variant="up" delay={140}>
				<FeatureColumns items={features} {accent} />
			</Reveal>
		</Container>
	</div>
</section>
