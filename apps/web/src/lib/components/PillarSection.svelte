<script lang="ts">
import { Container, FeatureColumns } from "$lib/components";
import type { FeatureColumn } from "$lib/components/FeatureColumns.svelte";
import type { IconComponent } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import SectionLabel from "./SectionLabel.svelte";

// The landing page's product-pillar template: a left-aligned label, heading,
// two-line description and one outlined action, then a full-bleed paper band
// holding the product visual, closed by a three-up detail row.
//
// Left-aligned on purpose. A centred stack reads as a slide; this reads as a
// page, and it gives the visual band the full column width beside it.
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
	<Container class="py-16 md:py-20">
		<div class="max-w-xl">
			<SectionLabel {icon} {label} {accent} />
			<h2 class="font-display text-balance mt-5 text-heading-lg md:text-display">
				{title}
			</h2>
			<p class="text-pretty mt-4 text-body-lg text-muted-foreground">
				{description}
			</p>
			{#if ctaHref && ctaLabel}
				<Button href={ctaHref} variant="outline" class="mt-8">{ctaLabel}</Button>
			{/if}
		</div>
	</Container>

	<div class="border-y border-border-low bg-paper">
		<Container class="py-12 md:py-16">
			{@render visual()}
		</Container>
		<Container>
			<FeatureColumns items={features} {accent} />
		</Container>
	</div>
</section>
