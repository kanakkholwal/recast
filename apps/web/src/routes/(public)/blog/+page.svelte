<script lang="ts">
	import { formatDate } from "$lib/blog/format";
	import { Container, Footer, HeroBackdrop, Reveal, Section, SeoMeta } from "$lib/components";
	import { ArrowRight, Clock } from "@lucide/svelte";
	import { Badge } from "@recast/ui/badge";
	import type { PageData } from "./$types";

	let { data }: { data: PageData } = $props();
</script>

<SeoMeta
	title="Engineering blog"
	description="How Recast is built. Long-form write-ups of the problems we hit, the decisions we made, and the ones we got wrong first."
	eyebrow="Blog"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-24">
		<HeroBackdrop src="/background-blog.webp" tone="strong" />
		<Container class="relative">
			<div class="relative z-10 mx-auto flex max-w-3xl flex-col items-start gap-7 md:items-center md:text-center">
				<h1
					class="text-balance animate-fade-up text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl"
				>
					How Recast
					<span class="mt-2 block font-medium italic text-foreground/40">gets built.</span>
				</h1>
				<p
					class="text-pretty max-w-xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg"
					style="animation-delay: 120ms"
				>
					Long-form write-ups from inside the work. The problems we hit, the decisions we made, and
					the ones we got wrong before we got them right.
				</p>
			</div>
		</Container>
	</Section>

	<Section spacing="tight" class="border-t border-border-low/60">
		<Container size="narrow">
			{#if data.posts.length === 0}
				<div class="glass-card rounded-2xl p-10 text-center">
					<h2 class="text-xl font-semibold tracking-tight text-foreground">Nothing published yet</h2>
					<p class="mt-2 text-sm text-muted-foreground">
						The first write-ups are on their way. Check back shortly.
					</p>
				</div>
			{:else}
				<ol class="space-y-6">
					{#each data.posts as post, i (post.slug)}
						<Reveal as="li" delay={i * 60}>
							<a
								href={post.url}
								class="glass-card group block rounded-2xl p-7 transition-all hover:-translate-y-0.5 hover:shadow-craft-sm focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
							>
								<article>
									<div class="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
										<time datetime={post.date}>{formatDate(post.date)}</time>
										<span class="text-muted-foreground/40">·</span>
										<span class="inline-flex items-center gap-1.5">
											<Clock class="size-3.5" />
											{post.readingMinutes} min read
										</span>
										{#if !post.published}
											<Badge variant="outline" class="border-warning/30 text-warning">Draft</Badge>
										{/if}
									</div>

									<h2
										class="mt-3 text-2xl font-semibold tracking-tight text-foreground transition-colors group-hover:text-primary"
									>
										{post.title}
									</h2>

									<p class="text-pretty mt-3 text-sm leading-relaxed text-muted-foreground">
										{post.description}
									</p>

									<div class="mt-5 flex flex-wrap items-center gap-2">
										{#each post.tags as tag (tag)}
											<Badge variant="secondary" class="font-normal">{tag}</Badge>
										{/each}
										<span
											class="ml-auto inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground transition-colors group-hover:text-foreground"
										>
											Read
											<ArrowRight
												class="size-3.5 transition-transform group-hover:translate-x-0.5"
											/>
										</span>
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
