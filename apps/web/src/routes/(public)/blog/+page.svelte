<script lang="ts">
import { ArrowRight, Clock, PenLine } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { formatDate } from "$lib/blog/format";
import { Container, Footer, Reveal, Section, SeoMeta } from "$lib/components";
import { prefersReducedMotion } from "$lib/motion-core";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

// Hero entrance: same 80ms stagger as the rest of the public pages.
// 460ms per element lands the whole ladder in well under a second.
const reduced = $derived(prefersReducedMotion());
const heroStagger = 80;
const riseM = (delay: number) =>
	reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };
</script>

<SeoMeta
	title="Engineering blog"
	description="How Recast is built. Long-form write-ups of the problems we hit, the decisions we made, and the ones we got wrong first."
	eyebrow="Blog"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-24">
		<Container class="relative">
			<div class="relative z-10 mx-auto flex max-w-3xl flex-col items-start gap-7 md:items-center md:text-center">
				<span
					in:fly={riseM(heroStagger * 0)}
					class="inline-flex items-center gap-2 text-body-sm font-medium text-muted-foreground"
				>
					<PenLine class="size-3 text-foreground/60" />
					Blog
				</span>
				<h1
					in:fly={riseM(heroStagger * 1)}
					class="font-display text-balance text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl"
				>
					How Recast
					<span class="mt-2 block font-medium italic text-muted-foreground">gets built.</span>
				</h1>
				<p
					in:fly={riseM(heroStagger * 2)}
					class="text-pretty max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg"
				>
					The problems we hit, the decisions we made, and the ones we got wrong first.
				</p>
			</div>
		</Container>
	</Section>

	<Section spacing="tight" class="border-t border-border-low">
		<Container size="narrow">
			{#if data.posts.length === 0}
				<div class="surface-lg p-10 text-center">
					<h2 class="font-display text-subheading font-bold tracking-tight text-foreground">Nothing published yet</h2>
					<p class="mt-2 text-sm text-muted-foreground">
						The first write-ups are on their way. Check back shortly.
					</p>
				</div>
			{:else}
				<ol class="space-y-4">
					{#each data.posts as post, i (post.slug)}
						<Reveal as="li" delay={i * 60}>
							<a
								href={post.url}
								class="surface group block p-6 transition-colors hover:border-border-strong focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
							>
								<article>
									<div class="flex flex-wrap items-center gap-2.5 text-caption text-muted-foreground">
										<time datetime={post.date}>{formatDate(post.date)}</time>
										<span class="text-border-strong">·</span>
										<span class="inline-flex items-center gap-1.5">
											<Clock class="size-3.5" />
											{post.readingMinutes} min read
										</span>
										{#if !post.published}
											<Badge variant="outline">Draft</Badge>
										{/if}
									</div>

									<div class="mt-3 flex items-start justify-between gap-4">
										<h2 class="font-display text-heading-sm font-bold tracking-tight text-foreground">
											{post.title}
										</h2>
										<ArrowRight
											class="mt-1.5 size-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5 motion-reduce:transition-none"
										/>
									</div>

									<p class="text-pretty mt-2 text-body-sm leading-relaxed text-muted-foreground">
										{post.description}
									</p>

									<div class="mt-4 flex flex-wrap items-center gap-2">
										{#each post.tags as tag (tag)}
											<Badge variant="secondary" class="font-normal">{tag}</Badge>
										{/each}
									</div>
								</article>
							</a>
						</Reveal>
					{/each}
				</ol>
			{/if}
		</Container>
	</Section>

	<Footer />
</main>
