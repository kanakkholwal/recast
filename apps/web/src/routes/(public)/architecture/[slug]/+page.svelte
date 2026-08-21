<script lang="ts">
import { ArrowLeft, ArrowRight, ShieldCheck } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { entrypointLabel } from "$lib/architecture/meta.logic";
import { DOMAIN_ACCENT, DOMAIN_LABEL, STATUS_LABEL } from "$lib/architecture/types";
import { Container, Footer, Section, SeoMeta } from "$lib/components";
import DocToc from "$lib/docs/DocToc.svelte";
import Prose from "$lib/docs/Prose.svelte";
import { prefersReducedMotion } from "$lib/motion-core";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

// Hero entrance: same 80ms stagger as the rest of the public pages.
const reduced = $derived(prefersReducedMotion());
const heroStagger = 80;
const riseM = (delay: number) =>
	reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };

const meta = $derived(data.meta);

const facts = $derived([
	{ term: "Takes in", items: meta.inputs },
	{ term: "Gives out", items: meta.outputs },
]);
</script>

<SeoMeta title={meta.title} description={meta.description} eyebrow="Architecture" />

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-32 pb-10 md:pt-40 md:pb-14">
		<Container>
			<a
				href="/architecture"
				class="inline-flex items-center gap-1.5 text-body-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
			>
				<ArrowLeft class="size-3.5" />
				All subsystems
			</a>

			<header class="mt-8 flex max-w-3xl flex-col gap-5">
				<div class="flex flex-wrap items-center gap-2">
					<span
						class="text-body-sm font-medium"
						class:text-tag-tangerine={DOMAIN_ACCENT[meta.domain] === "tangerine"}
						class:text-tag-lavender={DOMAIN_ACCENT[meta.domain] === "lavender"}
						class:text-tag-green={DOMAIN_ACCENT[meta.domain] === "green"}
						class:text-muted-foreground={DOMAIN_ACCENT[meta.domain] === "neutral"}
					>
						{DOMAIN_LABEL[meta.domain]}
					</span>
					{#if meta.status !== "production"}
						<Badge variant="outline">{STATUS_LABEL[meta.status]}</Badge>
					{/if}
				</div>

				<h1
					in:fly={riseM(heroStagger * 0)}
					class="font-display text-balance text-heading font-bold leading-[1.06] tracking-tight text-foreground sm:text-heading-lg md:text-display"
				>
					{meta.title}
				</h1>

				<p
					in:fly={riseM(heroStagger * 1)}
					class="text-pretty text-body-lg leading-relaxed text-muted-foreground"
				>
					{meta.summary}
				</p>
			</header>
		</Container>
	</Section>

	<Section spacing="none" class="pb-12 md:pb-16">
		<Container>
			<div class="surface-lg grid gap-px overflow-hidden bg-border-low md:grid-cols-3">
				{#each facts as fact (fact.term)}
					<div class="bg-card p-5">
						<h2 class="text-caption font-medium text-muted-foreground">{fact.term}</h2>
						<ul class="mt-2 flex flex-col gap-1 text-body-sm text-foreground">
							{#each fact.items as item (item)}
								<li>{item}</li>
							{/each}
						</ul>
					</div>
				{/each}

				<div class="bg-card p-5">
					<h2 class="text-caption font-medium text-muted-foreground">Start reading at</h2>
					<ul class="mt-2 flex flex-wrap gap-1.5">
						{#each meta.entrypoints as path (path)}
							<li>
								<!-- Full path in the tooltip: the chip shows only the leaf so a
								     row of them stays scannable. -->
								<span
									class="pill inline-block px-2 py-0.5 font-mono text-caption text-foreground"
									title={path}
								>
									{entrypointLabel(path)}
								</span>
							</li>
						{/each}
					</ul>
				</div>
			</div>

			<div class="surface-alt mt-4 p-5">
				<h2 class="flex items-center gap-2 text-caption font-medium text-muted-foreground">
					<ShieldCheck class="size-3.5" />
					Invariants
				</h2>
				<ul class="mt-3 flex flex-col gap-2">
					{#each meta.invariants as invariant (invariant)}
						<li class="flex gap-2.5 text-body-sm text-foreground">
							<span class="mt-2 size-1 shrink-0 rounded-full bg-border-strong"></span>
							<span class="text-pretty">{invariant}</span>
						</li>
					{/each}
				</ul>
			</div>
		</Container>
	</Section>

	<Section spacing="tight" class="border-t border-border-low">
		<Container>
			<div class="flex flex-col gap-10 lg:flex-row lg:gap-14">
				<!-- docvia compiled this to a plain node tree at build time, so there is
				     no markdown parser or syntax highlighter in the browser bundle. -->
				<div class="min-w-0 flex-1">
					<Prose nodes={data.content} width="reference" />
				</div>

				<DocToc headings={meta.headings} />
			</div>
		</Container>
	</Section>

	{#if data.previous || data.next}
		<Section spacing="tight" class="border-t border-border-low">
			<Container>
				<div class="grid gap-4 sm:grid-cols-2">
					{#if data.previous}
						<a
							href="/architecture/{data.previous.slug}"
							class="surface group flex flex-col gap-1 p-5 transition-colors hover:border-border-strong"
						>
							<span class="inline-flex items-center gap-1.5 text-caption text-muted-foreground">
								<ArrowLeft class="size-3" />
								Previous
							</span>
							<span class="font-display text-body font-bold tracking-tight text-foreground">
								{data.previous.title}
							</span>
						</a>
					{:else}
						<span></span>
					{/if}

					{#if data.next}
						<a
							href="/architecture/{data.next.slug}"
							class="surface group flex flex-col items-end gap-1 p-5 text-right transition-colors hover:border-border-strong sm:col-start-2"
						>
							<span class="inline-flex items-center gap-1.5 text-caption text-muted-foreground">
								Next
								<ArrowRight class="size-3" />
							</span>
							<span class="font-display text-body font-bold tracking-tight text-foreground">
								{data.next.title}
							</span>
						</a>
					{/if}
				</div>
			</Container>
		</Section>
	{/if}

	<Footer />
</main>
