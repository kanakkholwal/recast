<script lang="ts" module>
import type { IconComponent } from "@recast/icons";

export type PillarDetail = {
	icon: IconComponent;
	title: string;
	description: string;
	href?: string;
	linkLabel?: string;
};
</script>

<script lang="ts">
import { Button } from "@recast/ui/button";
import { Reveal } from "@recast/ui/reveal";
import { cn } from "@recast/ui/utils";
import type { Snippet } from "svelte";
import Container from "./Container.svelte";
import SectionLabel from "./SectionLabel.svelte";


let {
	id,
	index,
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
	/** Chapter number, e.g. "01". Rendered in the display face on the rule. */
	index: string;
	icon?: IconComponent;
	label: string;
	accent?: "tangerine" | "lavender" | "green" | "primary";
	title: string;
	description: string;
	ctaHref?: string;
	ctaLabel?: string;
	features: PillarDetail[];
	visual: Snippet;
	class?: string;
} = $props();

const glyph = {
	tangerine: "text-tag-tangerine",
	lavender: "text-tag-lavender",
	green: "text-tag-green",
	primary: "text-primary",
} as const;
</script>

<section {id} class={cn("relative", className)}>
	<Container>
		<Reveal variant="up">
			<div class="flex items-center gap-4 border-b border-border-low py-5">
				<span class="font-display text-heading-sm leading-none tabular-nums text-border-strong">
					{index}
				</span>
				<SectionLabel {icon} {label} {accent} />
				{#if ctaHref && ctaLabel}
					<Button href={ctaHref} variant="outline" size="sm" class="ml-auto shrink-0">
						{ctaLabel}
					</Button>
				{/if}
			</div>
		</Reveal>

		<div class="grid gap-10 py-14 md:grid-cols-12 md:gap-12 md:py-16">
			<div class="md:col-span-5">
				<Reveal variant="up" delay={60}>
					<h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
						{title}
					</h2>
				</Reveal>
				<Reveal variant="up" delay={120} class="mt-4">
					<p class="text-pretty text-body-md text-muted-foreground">
						{description}
					</p>
				</Reveal>
			</div>

			<!-- Details stack down the right column, divided by hairlines. -->
			<ul class="divide-y divide-border-low  md:col-span-6 md:col-start-7">
				{#each features as item, i (item.title)}
					{@const Icon = item.icon}
					<Reveal variant="up" delay={160 + i * 70} as="li" class="flex gap-4 py-5">
						<Icon
							class={cn("mt-0.5 size-5 shrink-0 [fill-opacity:0.2]", glyph[accent])}
							fill="currentColor"
						/>
						<div class="min-w-0">
							<h3 class="text-body font-semibold text-foreground">{item.title}</h3>
							<p class="mt-1 text-body-sm text-muted-foreground">{item.description}</p>
							{#if item.href}
								<a
									href={item.href}
									class="mt-2 inline-flex items-center gap-1 text-body-sm font-medium text-foreground underline-offset-4 hover:underline"
								>
									{item.linkLabel ?? "Learn more"}
									<span aria-hidden="true">›</span>
								</a>
							{/if}
						</div>
					</Reveal>
				{/each}
			</ul>
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
	</div>
</section>
