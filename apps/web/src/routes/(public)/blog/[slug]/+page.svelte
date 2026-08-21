<script lang="ts">
import { ArrowLeft, Clock } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { formatDate } from "$lib/blog/format";
import { Container, Footer, Section, SeoMeta } from "$lib/components";
import Prose from "$lib/docs/Prose.svelte";
import { prefersReducedMotion } from "$lib/motion-core";
import type { PageData } from "./$types";

// Hero entrance: same 80ms stagger as the rest of the public pages.
// 460ms per element lands the whole ladder in well under a second.
const reduced = $derived(prefersReducedMotion());
const heroStagger = 80;
const riseM = (delay: number) =>
	reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };

let { data }: { data: PageData } = $props();

const meta = $derived(data.meta);
</script>

<SeoMeta
	title={meta.title}
	description={meta.description}
	eyebrow="Blog"
	ogType="article"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-32 pb-10 md:pt-40 md:pb-14">
		<Container size="narrow">
			<a
				href="/blog"
				class="inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground transition-colors hover:text-foreground"
			>
				<ArrowLeft class="size-3.5" />
				All articles
			</a>

		<header class="mt-8 flex flex-col gap-6">
			<h1
				in:fly={riseM(heroStagger * 0)}
				class="text-balance text-3xl font-bold leading-[1.04] tracking-tight text-foreground sm:text-5xl md:text-6xl"
			>
				{meta.title}
			</h1>

			<p
				in:fly={riseM(heroStagger * 1)}
				class="text-pretty text-lg leading-relaxed text-muted-foreground"
			>
				{meta.description}
			</p>

				<div class="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
					{#if meta.author}
						<span class="font-semibold text-foreground">{meta.author}</span>
						<span class="text-border-strong">·</span>
					{/if}
					<time datetime={meta.date}>{formatDate(meta.date)}</time>
					<span class="text-border-strong">·</span>
					<span class="inline-flex items-center gap-1.5">
						<Clock class="size-3.5" />
						{meta.readingMinutes} min read
					</span>
					{#if !meta.published}
						<Badge variant="outline" class="border-warning/30 text-warning">Draft</Badge>
					{/if}
				</div>
			</header>
		</Container>
	</Section>

	<Section spacing="tight" class="border-t border-border-low">
		<Container size="narrow">
			<!-- docvia compiled this to a plain node tree at build time, so there is no
			     markdown parser or syntax highlighter in the browser bundle. -->
			<Prose nodes={data.content} />

			{#if meta.tags.length > 0}
				<div class="mt-14 flex flex-wrap items-center gap-2 border-t border-border-low pt-8">
					{#each meta.tags as tag (tag)}
						<Badge variant="secondary" class="font-normal">{tag}</Badge>
					{/each}
				</div>
			{/if}
		</Container>
	</Section>

	<Footer />
</main>

