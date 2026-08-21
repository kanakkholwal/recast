<script lang="ts">
import { Container, Footer, Reveal, Section, SectionHeader, SeoMeta } from "$lib/components";
import { prefersReducedMotion, TextLoop } from "$lib/motion-core";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import {
	gapRows,
	pillars,
	platforms,
	stabilityChip,
	stabilityChipOnFill,
	supports,
	verbs,
} from "./data";

// Hero entrance: same 80ms stagger as the rest of the public pages.
// 460ms per element lands the whole ladder in well under a second.
const reduced = $derived(prefersReducedMotion());
const heroStagger = 80;
const riseM = (delay: number) =>
	reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };

import { ArrowRight, Check } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
</script>

<SeoMeta
	title="Everything Recast does for you"
	description="Recording profiles, pause and resume, smart auto-zoom, cursor smoothing, silence cuts, on-frame annotations, Drive sharing. The full feature catalog."
	eyebrow="Features"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-20 md:pt-48 md:pb-24">
		<Container class="relative">
			<div class="relative z-10 mx-auto flex max-w-3xl flex-col items-center gap-7 text-center">
				<span
					in:fly={riseM(heroStagger * 0)}
					class="inline-flex items-center gap-2 text-body-sm font-medium text-muted-foreground"
				>
					<span class="size-1.5 rounded-full bg-primary"></span>
					Features
				</span>
				<h1
					in:fly={riseM(heroStagger * 1)}
					class="text-balance text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5rem]"
				>
					Everything Recast
					<span class="mt-2 flex justify-center font-medium italic text-muted-foreground">
						<span class="inline-grid overflow-hidden">
							<TextLoop class="text-primary" texts={verbs} interval={2800} />
						</span>
					</span>
				</h1>
				<p
					in:fly={riseM(heroStagger * 2)}
					class="text-pretty max-w-2xl text-base leading-relaxed text-muted-foreground sm:text-lg"
				>
					A focused recorder for solo founders, indie hackers, and product engineers who'd rather ship than fiddle. Auto-polish for the 80% case, a minimal timeline for the moments you want to control.
				</p>

				<!-- Platform chips: honest about per-platform maturity. -->
				<ul class="mt-2 flex flex-wrap items-center justify-center gap-2 text-caption font-semibold">
					{#each platforms as p (p.label)}
						{@const Icon = p.icon}
						{@const chip = stabilityChip[p.stability]}
						<li class="inline-flex items-center gap-2 rounded-full border border-border-low bg-card/40 px-3 py-1.5 text-foreground ring-1 ring-inset ring-border-low">
							<Icon class="size-3.5" />
							{p.label}
							<span class={cn("ml-0.5 inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-caption font-bold font-medium ring-1 ring-inset", chip.cls)}>
								{chip.label}
							</span>
						</li>
					{/each}
				</ul>
			</div>
		</Container>
	</Section>

	<!-- Three pillars: lead with differentiators. -->
	<Section spacing="tight" class="border-t border-border-low">
		<Container>
			<div class="grid gap-4 lg:grid-cols-3">
				{#each pillars as pillar, i}
					{@const Icon = pillar.icon}
					<Reveal delay={i * 80}>
						<article class="bg-card group relative flex h-full flex-col overflow-hidden rounded-2xl p-8 transition-all duration-300 hover:">
							<span class="pill grid size-12 place-items-center rounded-xl text-muted-foreground transition-all group-hover:scale-105 group-hover:text-primary">
								<Icon class="size-5" />
							</span>
							<h3 class="mt-6 text-xl font-semibold tracking-tight text-foreground">
								{pillar.title}
							</h3>
							<p class="mt-2.5 text-pretty text-sm leading-relaxed text-muted-foreground">
								{pillar.description}
							</p>
							<ul class="mt-6 flex flex-wrap gap-2">
								{#each pillar.tags as tag}
									<li class="pill rounded-full px-2.5 py-1 text-caption font-medium text-muted-foreground">
										{tag}
									</li>
								{/each}
							</ul>
						</article>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- "Free here, paid elsewhere." Concrete value-gap table. Compares
	     against the two products we get compared to most, with conservative
	     claims and a direct tone. -->
	<Section class="border-t border-border-low">
		<Container>
			<SectionHeader
				eyebrow="Side by side"
				title="Free here. Paid in the others."
				description="Most of what Recast ships in the free desktop app is either paywalled or missing in the products we get compared to most. The honest version."
				align="center"
			/>

			<Reveal variant="blur" class="mt-14">
				<div class="overflow-x-auto rounded-2xl border border-border-low">
					<div class="min-w-160">
						<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr] border-b border-border-low bg-paper text-body-sm font-medium">
							<div class="px-5 py-3.5 text-muted-foreground">Feature</div>
							<div class="border-l border-border-low px-5 py-3.5 text-center text-primary">Recast</div>
							<div class="border-l border-border-low px-5 py-3.5 text-center text-foreground">Loom</div>
							<div class="border-l border-border-low px-5 py-3.5 text-center text-foreground">Cap</div>
						</div>
						{#each gapRows as row, i}
							<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr] {i < gapRows.length - 1 ? 'border-b border-border-low' : ''}">
								<div class="px-5 py-3.5 text-sm text-foreground">{row.feature}</div>
								<div class="flex items-center justify-center border-l border-border-low bg-primary/4 px-5 py-3.5 text-center">
									<span class="inline-flex items-center gap-1.5 text-xs font-semibold text-foreground">
										<Check class="size-3.5 text-primary" />
										{row.recast}
									</span>
								</div>
								<div class="flex items-center justify-center border-l border-border-low px-5 py-3.5 text-center text-xs text-muted-foreground">
									{row.loom}
								</div>
								<div class="flex items-center justify-center border-l border-border-low px-5 py-3.5 text-center text-xs text-muted-foreground">
									{row.cap}
								</div>
							</div>
						{/each}
					</div>
				</div>
			</Reveal>

			<Reveal variant="up" class="mt-6">
				<p class="mx-auto max-w-2xl text-balance text-center text-xs leading-relaxed text-muted-foreground">
					Comparison is based on the publicly documented tiers of each product. Got a correction? Open an issue on
					<a href="https://github.com/kanakkholwal/recast" class="text-foreground underline-offset-2 hover:underline">GitHub</a>.
				</p>
			</Reveal>
		</Container>
	</Section>

	<!--
	  Full catalog, vendor layout. Every third card (1, 4, 7, 10, 13, 16)
	  spans 2 cols for visual rhythm and to highlight the marquee features
	  (auto-zoom, pause-and-resume, hardware export, native capture,
	  offline-first, open-source). Each card renders its `image` when one
	  exists, with the feature icon as a tinted placeholder until then. The
	  small "tag" badge sits in the screenshot's bottom-right corner (the
	  module name: Capture, Edit, Export, Privacy…). On lg the big card
	  lays out horizontally (image left, text right); normal cards stack
	  vertically. On mobile every card drops to 1 col.
	-->
	<Section class="border-t border-border-low">
		<Container>
			<SectionHeader
				eyebrow="Built in"
				title="The full catalog."
				description="Every affordance worth naming, in one grid. All shipping in the free desktop app today."
			/>

			<div class="mt-14 grid auto-rows-fr grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
				{#each supports as item, i}
					{@const Icon = item.icon}
					{@const isFeatured = i % 3 === 0}
					<Reveal delay={i * 35} class={isFeatured ? "lg:col-span-2" : ""}>
						<article
							class={cn(
								"surface-lg group relative flex h-full overflow-hidden border border-border-low bg-card/40 transition-[transform,box-shadow,border-color] duration-300 hover:border-border-low hover:shadow-craft-sm motion-reduce:transition-none",
								isFeatured
									? "flex-col p-0 lg:flex-row lg:items-stretch"
									: "flex-col p-5",
							)}
						>
							<!--
							  Visual panel. Real screenshot when one exists, otherwise a
							  tinted icon-as-hero placeholder the same dimensions so the
							  card stays stable once a real image drops in. The "tag" badge
							  anchors to the bottom-right of the visual.
							-->
							<div
								class={cn(
									"relative overflow-hidden",
									isFeatured
										? "aspect-4/3 shrink-0 border-b border-border-low lg:aspect-auto lg:w-1/2 lg:border-b-0 lg:border-r"
										: "aspect-16/10 w-full",
								)}
							>
								{#if item.image}
									<img
										src={item.image}
										alt={item.title}
										loading="lazy"
										decoding="async"
										class="absolute inset-0 size-full object-cover"
									/>
								{:else}
									<!-- Tinted placeholder. Soft radial wash from the top so the
									     placeholder reads as a "screenshot slot" rather than a
									     missing image. -->
									<div
										class="absolute inset-0"
										style="background: linear-gradient(160deg, color-mix(in srgb, var(--color-foreground) 7%, transparent) 0%, color-mix(in srgb, var(--color-foreground) 3%, transparent) 60%, transparent 100%);"
									></div>
									<div
										class="absolute inset-0 grid place-items-center"
									>
										<div
											class={cn(
												"grid place-items-center rounded-2xl border border-border-low bg-card/60 shadow-craft-sm",
												isFeatured ? "size-20" : "size-14",
											)}
										>
											<Icon
												class={cn(
													"text-muted-foreground",
													isFeatured ? "size-9" : "size-6",
												)}
											/>
										</div>
									</div>
								{/if}

								<!--
								  Module tag (Capture, Edit, Export, …). Pinned to the
								  bottom-right of the visual. Mirrors the "Notion / Google /
								  OpenAI" brand badges on the reference vendor layout.
								-->
								<span
									class="absolute bottom-2.5 right-2.5 inline-flex items-center gap-1 rounded-full bg-foreground/85 px-1.5 py-0.5 text-caption font-semibold font-medium text-background shadow-craft-sm"
								>
									{item.tag}
								</span>
							</div>

							<!--
							  Text panel. Title + description + "Learn more" link. Padded
							  inside the visual border so the card stays one piece.
							-->
							<div
								class={cn(
									"flex min-w-0 flex-1 flex-col",
									isFeatured ? "gap-3 p-5 sm:p-6 lg:gap-4 lg:p-8" : "mt-5 gap-2",
								)}
							>
								<h3
									class={cn(
										"font-semibold tracking-tight text-foreground",
										isFeatured
											? "text-lg sm:text-xl lg:text-2xl"
											: "text-sm",
									)}
								>
									{item.title}
								</h3>
								<p
									class={cn(
										"text-pretty leading-relaxed text-muted-foreground",
										isFeatured
											? "text-sm sm:text-base"
											: "text-xs",
									)}
								>
									{item.description}
								</p>
								<a
									href={item.href}
									class={cn(
										"mt-auto inline-flex items-center gap-1 self-start text-body-sm font-medium text-muted-foreground transition-colors hover:text-foreground",
										isFeatured ? "pt-2 text-xs" : "pt-1",
									)}
								>
									Learn more
									<ArrowRight class="size-3 transition-transform group-hover:translate-x-0.5" />
								</a>
							</div>
						</article>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Final CTA: platform-split downloads, same pattern as the landing page. -->
	<Section id="cta" class="border-t border-border-low">
		<Container>
			<Reveal>
				<div
					class="surface-lg relative overflow-hidden rounded-[2rem] px-6 py-16 sm:px-14 sm:py-20 md:py-24"
					style="box-shadow: inset 0 1px 0 0 color-mix(in srgb, white 12%, transparent), inset 0 -1px 0 0 color-mix(in srgb, var(--color-foreground) 4%, transparent);"
				>
			

					<div class="relative mx-auto flex max-w-3xl flex-col items-center text-center">
						<div class="pill inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-body-sm font-medium text-foreground">
							<span class="relative flex size-1.5">
								<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-70"></span>
								<span class="relative inline-flex size-1.5 rounded-full bg-primary"></span>
							</span>
							ready when you are
						</div>

						<h2 class="text-balance mt-8 text-4xl font-semibold leading-[1.02] tracking-tight text-foreground sm:text-5xl md:text-6xl lg:text-[4.25rem]">
							Skip the editor.
							<span class="block font-medium italic text-muted-foreground">Ship the demo.</span>
						</h2>

						<p class="text-pretty mt-7 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Free forever. No account. Windows is daily-driver stable, macOS and Linux are in active beta.
						</p>

						<div class="mt-10 flex w-full flex-col items-stretch gap-3 sm:w-auto sm:flex-row sm:flex-wrap sm:items-center sm:justify-center sm:gap-3">
							{#each platforms as p}
								{@const Icon = p.icon}
								{@const chip = stabilityChip[p.stability]}
								{@const isPrimary = p.stability === "stable"}
								<Button
									href={`/download?os=${p.label.toLowerCase()}`}
									size="lg"
									variant={isPrimary ? "default" : "outline"}
									class="gap-2.5"
								>
									<Icon class="size-4" />
									Download for {p.label}
									<span class={cn("ml-1 inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-caption font-semibold ring-1 ring-inset", stabilityChipOnFill)}>
										{chip.label}
									</span>
								</Button>
							{/each}
						</div>

						<a
							href="/changelog"
							class="group/cta mt-5 inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground transition-colors hover:text-foreground"
						>
							See what's
							<ArrowRight class="size-3.5 transition-transform group-hover/cta:translate-x-0.5" />
						</a>
					</div>
				</div>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
