<script lang="ts">
import { Container, Footer, Section, SeoMeta } from "$lib/components";
import { formatDate } from "$lib/blog/format";
import { prefersReducedMotion } from "$lib/motion-core";
import { Badge } from "@recast/ui/badge";
import DocviaContent from "$lib/blog/DocviaContent.svelte";
import { ArrowLeft, Clock } from "@recast/icons";
import { fly } from "svelte/transition";
import { cubicOut } from "svelte/easing";
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
			<article class="prose">
				<DocviaContent nodes={data.content} />
			</article>

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

<style>
	/* The Renderer emits its own elements, so Svelte's scoped classes never land
	   on them. Style them through `:global()` under this wrapper, and take every
	   colour from a design token so the article tracks the active theme. */
	.prose {
		color: var(--foreground);
		font-size: 1.0625rem;
		line-height: 1.75;
		max-width: 68ch;
	}

	.prose :global(> :first-child) {
		margin-top: 0;
	}
	.prose :global(> :last-child) {
		margin-bottom: 0;
	}

	.prose :global(p) {
		margin: 0 0 1.4em;
		text-wrap: pretty;
	}

	.prose :global(h2),
	.prose :global(h3),
	.prose :global(h4) {
		color: var(--foreground);
		font-weight: 600;
		letter-spacing: -0.015em;
		line-height: 1.25;
		text-wrap: balance;
		/* Clears the fixed navbar when a heading anchor is jumped to. */
		scroll-margin-top: 6rem;
	}
	.prose :global(h2) {
		margin: 2.4em 0 0.8em;
		font-size: 1.65em;
	}
	.prose :global(h3) {
		margin: 2em 0 0.6em;
		font-size: 1.3em;
	}
	.prose :global(h4) {
		margin: 1.8em 0 0.5em;
		font-size: 1.1em;
	}

	.prose :global(strong) {
		font-weight: 650;
		color: var(--foreground);
	}
	.prose :global(em) {
		font-style: italic;
	}

	.prose :global(a) {
		color: var(--primary);
		text-decoration: underline;
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
		text-decoration-color: color-mix(in oklab, var(--primary) 40%, transparent);
		transition: text-decoration-color 150ms ease;
	}
	.prose :global(a:hover) {
		text-decoration-color: var(--primary);
	}

	.prose :global(ul),
	.prose :global(ol) {
		margin: 0 0 1.4em;
		padding-left: 1.5em;
	}
	.prose :global(ul) {
		list-style: disc;
	}
	.prose :global(ol) {
		list-style: decimal;
	}
	.prose :global(li) {
		margin: 0.4em 0;
		padding-left: 0.2em;
	}
	.prose :global(li::marker) {
		color: var(--muted-foreground);
	}

	.prose :global(code) {
		font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
		font-size: 0.875em;
		padding: 0.15em 0.4em;
		border-radius: 5px;
		background: color-mix(in oklab, var(--muted) 70%, transparent);
		color: var(--foreground);
		overflow-wrap: anywhere;
	}
	/* Shiki bakes the highlighted markup (and its own background) into the block
	   at build time, so only the frame is ours. */
	.prose :global(pre) {
		margin: 0 0 1.6em;
		padding: 1.1em 1.25em;
		border-radius: 12px;
		border: 1px solid var(--border);
		overflow-x: auto;
		font-size: 0.9em;
		line-height: 1.6;
	}
	.prose :global(pre code) {
		padding: 0;
		background: none;
		font-size: inherit;
	}

	.prose :global(blockquote) {
		margin: 0 0 1.4em;
		padding: 0.3em 0 0.3em 1.1em;
		border-left: 3px solid color-mix(in oklab, var(--primary) 50%, transparent);
		color: var(--muted-foreground);
		font-style: italic;
	}

	.prose :global(hr) {
		margin: 2.5em 0;
		border: none;
		border-top: 1px solid var(--border);
	}

	.prose :global(img) {
		max-width: 100%;
		height: auto;
		border-radius: 12px;
		border: 1px solid var(--border);
	}

	.prose :global(table) {
		width: 100%;
		margin: 0 0 1.6em;
		border-collapse: collapse;
		font-size: 0.92em;
		display: block;
		overflow-x: auto;
	}
	.prose :global(th),
	.prose :global(td) {
		padding: 0.55em 0.8em;
		border: 1px solid var(--border);
		text-align: left;
		vertical-align: top;
	}
	.prose :global(th) {
		font-weight: 600;
		background: color-mix(in oklab, var(--muted) 50%, transparent);
	}
</style>
