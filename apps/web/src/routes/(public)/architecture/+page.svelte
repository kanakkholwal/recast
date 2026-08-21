<script lang="ts">
import type { IconComponent } from "@recast/icons";
import { ArrowRight, Cloud, Cpu, Film, GitGraph, Layers, Package, Video } from "@recast/icons";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { groupByDomain } from "$lib/architecture/meta.logic";
import SystemMap from "$lib/architecture/SystemMap.svelte";
import { SYSTEM_NODES } from "$lib/architecture/system-map";
import {
	type ArchitectureDomain,
	type ArchitectureMeta,
	DOMAIN_ACCENT,
	DOMAIN_LABEL,
} from "$lib/architecture/types";
import { Container, Footer, Reveal, Section, SectionLabel, SeoMeta } from "$lib/components";
import { prefersReducedMotion } from "$lib/motion-core";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

// Hero entrance: same 80ms stagger as the rest of the public pages.
// 460ms per element lands the whole ladder in well under a second.
const reduced = $derived(prefersReducedMotion());
const heroStagger = 80;
const riseM = (delay: number) =>
	reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };

const sections = $derived(groupByDomain(data.docs));

const DOMAIN_ICON: Record<ArchitectureDomain, IconComponent> = {
	platform: Layers,
	capture: Video,
	editor: Film,
	render: Cpu,
	pipeline: Package,
	cloud: Cloud,
	agent: GitGraph,
};

const PHASE_LEGEND = [
	{ label: "Record", class: "bg-tag-tangerine" },
	{ label: "Polish", class: "bg-tag-lavender" },
	{ label: "Share", class: "bg-tag-green" },
	{ label: "Artifact", class: "bg-border-strong" },
];

/** The map's own node list, doubling as its accessible and no-JS equivalent. */
const mapNodes = SYSTEM_NODES;

function href(doc: ArchitectureMeta): string {
	return `/architecture/${doc.slug}`;
}
</script>

<SeoMeta
	title="How Recast is built"
	description="One page per subsystem: what goes in, what comes out, and the invariants it cannot break."
	eyebrow="Architecture"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-14 md:pt-48 md:pb-20">
		<Container class="relative">
			<div class="relative z-10 mx-auto flex max-w-3xl flex-col items-start gap-7 md:items-center md:text-center">
				<span
					in:fly={riseM(heroStagger * 0)}
					class="inline-flex items-center gap-2 text-body-sm font-medium text-muted-foreground"
				>
					<Layers class="size-3.5 text-foreground/60" />
					Architecture
				</span>
				<h1
					in:fly={riseM(heroStagger * 1)}
					class="font-display text-balance text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl"
				>
					How Recast
					<span class="mt-2 block font-medium italic text-muted-foreground">actually works.</span>
				</h1>
				<p
					in:fly={riseM(heroStagger * 2)}
					class="text-pretty max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg"
				>
					One page per subsystem. What goes in, what comes out, and the rules it cannot break.
				</p>
			</div>
		</Container>
	</Section>

	<Section spacing="none" class="pb-16 md:pb-24">
		<Container>
			<Reveal>
				<div class="mb-4 flex flex-wrap items-end justify-between gap-3">
					<div>
						<h2 class="font-display text-subheading font-bold tracking-tight text-foreground">
							The whole system
						</h2>
						<p class="mt-1 text-body-sm text-muted-foreground">
							Drag to pan, click a box to open its page.
						</p>
					</div>
					<ul class="flex flex-wrap items-center gap-x-4 gap-y-2 text-caption text-muted-foreground">
						{#each PHASE_LEGEND as phase (phase.label)}
							<li class="inline-flex items-center gap-1.5">
								<span class="size-1.5 rounded-full {phase.class}"></span>
								{phase.label}
							</li>
						{/each}
					</ul>
				</div>

				<SystemMap />

				<!-- The map is a picture; this is the same graph as text, and the only
				     version a screen reader or a JS-less browser gets. -->
				<details class="mt-3">
					<summary
						class="cursor-pointer text-body-sm text-muted-foreground transition-colors hover:text-foreground"
					>
						Read the map as a list
					</summary>
					<ul class="mt-3 grid gap-x-8 gap-y-1.5 sm:grid-cols-2 lg:grid-cols-3">
						{#each mapNodes as node (node.id)}
							<li class="text-body-sm">
								{#if node.slug}
									<a
										class="font-medium text-foreground underline-offset-4 hover:underline"
										href="/architecture/{node.slug}"
									>
										{node.label}
									</a>
								{:else}
									<span class="font-medium text-foreground">{node.label}</span>
								{/if}
								<span class="text-muted-foreground"> · {node.runtime}</span>
							</li>
						{/each}
					</ul>
				</details>
			</Reveal>
		</Container>
	</Section>

	{#each sections as section (section.domain)}
		{@const Icon = DOMAIN_ICON[section.domain]}
		<Section spacing="tight" class="border-t border-border-low">
			<Container>
				<Reveal>
					<SectionLabel
						icon={Icon}
						label={DOMAIN_LABEL[section.domain]}
						accent={DOMAIN_ACCENT[section.domain]}
					/>
				</Reveal>

				<ul class="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
					{#each section.docs as doc, index (doc.slug)}
						<Reveal delay={index * 70}>
							<li class="h-full">
								<a
									href={href(doc)}
									class="surface group flex h-full flex-col gap-2.5 p-5 transition-colors hover:border-border-strong focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
								>
									<div class="flex items-start justify-between gap-3">
										<h3 class="font-display text-body-lg font-bold tracking-tight text-foreground">
											{doc.title}
										</h3>
										<ArrowRight
											class="mt-1 size-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5 motion-reduce:transition-none"
										/>
									</div>

									<p class="text-pretty flex-1 text-body-sm text-muted-foreground">{doc.summary}</p>

									<p class="border-t border-border-low pt-2.5 text-caption text-muted-foreground">
										{doc.entrypoints.length} entrypoints · {doc.invariants.length} invariants
									</p>
								</a>
							</li>
						</Reveal>
					{/each}
				</ul>
			</Container>
		</Section>
	{/each}

	<Footer />
</main>
