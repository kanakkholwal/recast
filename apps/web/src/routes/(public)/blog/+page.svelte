<script lang="ts">
import { ArrowRight, Clock, PenLine } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { formatDate } from "$lib/blog/format";
import { Container, Footer, Reveal, Section, SectionLabel, SeoMeta } from "$lib/components";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

// Newest first, so the lead card is always the latest write-up.
const posts = $derived(
	[...data.posts].sort((a, b) => (a.date < b.date ? 1 : a.date > b.date ? -1 : 0)),
);
const featured = $derived(posts[0]);
const rest = $derived(posts.slice(1));
</script>

<SeoMeta
	title="Recast blog"
	description="Field notes from building Recast, plus guides for recording product demos worth watching. Engineering deep-dives, tool comparisons, and the workflow that turns a raw take into something shippable."
	eyebrow="Blog"
/>

<main class="text-foreground">
	<!-- Interior-page hero: left-aligned SectionLabel, display h1, one line, then a
	     hairline rule carrying the page's facts. Same shape as /pricing, /download. -->
	<Section spacing="none" class="pt-36 pb-12 md:pt-44 md:pb-16">
		<Container>
			<div class="max-w-3xl">
				<Reveal variant="up">
					<SectionLabel icon={PenLine} label="Blog" accent="neutral" />
				</Reveal>
				<Reveal variant="up" delay={60}>
					<h1
						class="mt-5 font-display text-balance text-heading-lg font-bold leading-[1.04] tracking-tight text-foreground md:text-display"
					>
						Building Recast in the open.
					</h1>
				</Reveal>
				<Reveal variant="up" delay={120}>
					<p class="text-pretty mt-5 text-body-lg leading-relaxed text-muted-foreground">
						The problems we hit and the calls we made, plus guides for recording demos worth
						watching. Written by the people shipping it.
					</p>
				</Reveal>
			</div>
		</Container>

		{#if posts.length > 0}
			<Container class="mt-10">
				<div class="flex flex-wrap items-center gap-x-2.5 gap-y-1 border-t border-border-low pt-5 text-caption text-muted-foreground">
					<span class="tabular-nums text-foreground">{posts.length}</span>
					<span>{posts.length === 1 ? "article" : "articles"}</span>
					<span class="text-border-strong">·</span>
					<span>Engineering deep-dives and demo playbooks</span>
				</div>
			</Container>
		{/if}
	</Section>

	<Section spacing="tight" class="border-t border-border-low">
		<Container>
			{#if posts.length === 0}
				<div class="border-y border-border-low bg-paper px-6 py-16 text-center">
					<h2 class="font-display text-subheading font-bold tracking-tight text-foreground">
						Nothing published yet
					</h2>
					<p class="mt-2 text-body-sm text-muted-foreground">
						The first write-ups are on their way. Check back shortly.
					</p>
				</div>
			{:else}
				<!-- One gap-px hairline grid: the dividers are the site's rule, so the
				     listing reads as a ruled sheet, not a stack of floating cards. The
				     lead article spans both columns and sets larger type. -->
				<div class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low md:grid-cols-2">
					<Reveal
						variant="up"
						as="article"
						class="bg-background md:col-span-2"
					>
						<a
							href={featured.url}
							class="group flex h-full flex-col gap-5 p-6 transition-colors hover:bg-paper focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset md:p-8"
						>
							<div class="flex flex-wrap items-center gap-2.5 text-caption text-muted-foreground">
								<span class="pill px-2 py-0.5 text-caption font-medium text-foreground">Latest</span>
								<time datetime={featured.date}>{formatDate(featured.date)}</time>
								<span class="text-border-strong">·</span>
								<span class="inline-flex items-center gap-1.5">
									<Clock class="size-3.5" />
									{featured.readingMinutes} min read
								</span>
								{#if !featured.published}
									<Badge variant="outline">Draft</Badge>
								{/if}
							</div>

							<div class="flex items-start justify-between gap-6">
								<h2
									class="font-display text-balance text-heading-sm font-bold leading-tight tracking-tight text-foreground md:text-heading"
								>
									{featured.title}
								</h2>
								<ArrowRight
									class="mt-1 size-5 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-1 motion-reduce:transition-none"
								/>
							</div>

							<p class="text-pretty max-w-2xl text-body leading-relaxed text-muted-foreground">
								{featured.description}
							</p>

							{#if featured.tags.length > 0}
								<div class="mt-auto flex flex-wrap items-center gap-2 pt-1">
									{#each featured.tags.slice(0, 4) as tag (tag)}
										<Badge variant="secondary" class="font-normal">{tag}</Badge>
									{/each}
								</div>
							{/if}
						</a>
					</Reveal>

					{#each rest as post, i (post.slug)}
						<Reveal variant="up" delay={(i + 1) * 60} as="article" class="bg-background">
							<a
								href={post.url}
								class="group flex h-full flex-col gap-4 p-6 transition-colors hover:bg-paper focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
							>
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

								<div class="flex items-start justify-between gap-4">
									<h3
										class="font-display text-body-lg font-bold leading-snug tracking-tight text-foreground md:text-heading-sm"
									>
										{post.title}
									</h3>
									<ArrowRight
										class="mt-1 size-4 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5 motion-reduce:transition-none"
									/>
								</div>

								<p class="text-pretty line-clamp-3 text-body-sm leading-relaxed text-muted-foreground">
									{post.description}
								</p>

								{#if post.tags.length > 0}
									<div class="mt-auto flex flex-wrap items-center gap-2 pt-1">
										{#each post.tags.slice(0, 3) as tag (tag)}
											<Badge variant="secondary" class="font-normal">{tag}</Badge>
										{/each}
									</div>
								{/if}
							</a>
						</Reveal>
					{/each}
				</div>
			{/if}
		</Container>
	</Section>

	<Footer />
</main>
