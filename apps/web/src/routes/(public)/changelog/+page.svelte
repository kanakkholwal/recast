<script lang="ts">
import { ArrowUpRight, ExternalLink, GitCommit, Tag } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Markdown } from "@recast/ui/markdown";
import { cn } from "@recast/ui/utils";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { Container, Footer, Reveal, Section, SectionLabel, SeoMeta } from "$lib/components";
import { prefersReducedMotion } from "$lib/motion-core";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

const reduced = $derived(prefersReducedMotion());
const releases = $derived(data.releases);

let selected = $state(0);
const current = $derived(releases[selected] ?? releases[0]);

// Deep links: /changelog#v0.3.1 opens that release, and selecting one updates
// the hash so the URL is shareable.
$effect(() => {
	const tag = decodeURIComponent(location.hash.replace(/^#/, ""));
	if (!tag) return;
	const i = releases.findIndex((r) => r.tag === tag);
	if (i >= 0) selected = i;
});

function select(i: number) {
	selected = i;
	const tag = releases[i]?.tag;
	if (tag) history.replaceState(null, "", `#${encodeURIComponent(tag)}`);
}

// Roving arrow keys: a list this dense should not need the mouse.
function onKey(e: KeyboardEvent) {
	const delta = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
	if (!delta) return;
	e.preventDefault();
	const next = Math.min(releases.length - 1, Math.max(0, selected + delta));
	select(next);
	document.getElementById(`release-${next}`)?.focus();
}

function formatDate(iso: string | null) {
	if (!iso) return "";
	try {
		return new Intl.DateTimeFormat("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric",
		}).format(new Date(iso));
	} catch {
		return "";
	}
}
</script>

<SeoMeta
	title="Changelog"
	description="Every Recast release. Notes, fixes, and what shipped."
	eyebrow="Changelog"
/>

<main class="text-foreground">
	<section class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40">
		<Container class="pb-12">
			<Reveal variant="up">
				<SectionLabel icon={GitCommit} label="Changelog" />
			</Reveal>
			<Reveal variant="up" delay={60} class="mt-5">
				<h1 class="max-w-2xl font-display font-semibold text-balance text-heading-lg md:text-display">
					Every release, in order
				</h1>
			</Reveal>
			<Reveal variant="up" delay={120} class="mt-4">
				<p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
					Pulled live from GitHub releases. Newest first, notes and all.
				</p>
			</Reveal>
			<Reveal variant="up" delay={180} class="mt-8">
				<Button
					href="https://github.com/kanakkholwal/recast/releases"
					variant="outline"
					class="gap-2"
					target="_blank"
				>
					View on GitHub
					<ExternalLink class="size-3.5" />
				</Button>
			</Reveal>
		</Container>
	</section>

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			{#if releases.length === 0}
				<div class="border-y border-border-low py-16 text-center">
					<h2 class="font-display text-heading-sm text-foreground">No releases yet</h2>
					<p class="mt-2 text-body-sm text-muted-foreground">
						Recast is in early beta. Check back shortly, or watch the repo on GitHub.
					</p>
				</div>
			{:else}
				<div class="grid gap-px border-y border-border-low bg-border-low md:grid-cols-12">
					<!-- Rail -->
					<div class="bg-background md:col-span-4">
						<ul class="flex flex-col divide-y divide-border-low">
							{#each releases as release, i (release.tag)}
								{@const active = i === selected}
								<Reveal variant="up" delay={i * 45} as="li">
									<button
										id="release-{i}"
										type="button"
										aria-current={active ? "true" : undefined}
										onclick={() => select(i)}
										onkeydown={onKey}
										class={cn(
											"group/rel flex w-full items-center gap-3 px-5 py-4 text-left transition-colors duration-300 motion-reduce:transition-none",
											active
												? "bg-paper text-foreground"
												: "text-muted-foreground hover:bg-paper hover:text-foreground",
										)}
									>
										<Tag
											class={cn(
												"size-4 shrink-0 transition-colors duration-300 motion-reduce:transition-none",
												active ? "text-tag-green" : "text-border-strong",
											)}
										/>
										<span class="min-w-0 flex-1">
											<span class="block truncate text-body-sm font-medium">{release.name}</span>
											<span class="block truncate text-caption text-muted-foreground">
												{formatDate(release.publishedAt)}
												{#if release.prerelease}
													· Pre-release
												{/if}
											</span>
										</span>
										<ArrowUpRight
											class={cn(
												"size-4 shrink-0 transition-all duration-300 motion-reduce:transition-none",
												active
													? "translate-x-0 opacity-100"
													: "-translate-x-1 opacity-0 group-hover/rel:translate-x-0 group-hover/rel:opacity-60",
											)}
										/>
									</button>
								</Reveal>
							{/each}
						</ul>
					</div>

					<!-- Body -->
					<div class="bg-background md:col-span-8">
						{#key current?.tag}
							<article
								class="p-6 sm:p-8"
								in:fly={reduced
									? { duration: 0 }
									: { y: 10, duration: 320, easing: cubicOut }}
							>
								<header class="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-border-low pb-5">
									<h2 class="font-display text-heading-sm text-foreground">{current.name}</h2>
									{#if current.prerelease}
										<span
											class="rounded-full bg-tag-tangerine/12 px-2 py-0.5 text-caption font-medium text-tag-tangerine"
										>
											Pre-release
										</span>
									{:else if selected === 0}
										<span
											class="rounded-full bg-tag-green/12 px-2 py-0.5 text-caption font-medium text-tag-green"
										>
											Latest
										</span>
									{/if}
									<a
										href={current.url}
										target="_blank"
										rel="noopener noreferrer"
										class="ml-auto inline-flex items-center gap-1.5 text-body-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
									>
										GitHub
										<ArrowUpRight class="size-3.5" />
									</a>
								</header>

								<div class="mt-4 flex flex-wrap items-center gap-3 text-caption text-muted-foreground">
									<span class="inline-flex items-center gap-1.5">
										<GitCommit class="size-3.5" />
										{current.tag}
									</span>
									{#if current.publishedAt}
										<span class="text-border-strong">·</span>
										<time datetime={current.publishedAt}>{formatDate(current.publishedAt)}</time>
									{/if}
								</div>

								{#if current.body}
									<Markdown source={current.body} class="mt-6 text-body-sm text-foreground" />
								{:else}
									<p class="mt-6 text-body-sm text-muted-foreground">
										No release notes provided.
									</p>
								{/if}
							</article>
						{/key}
					</div>
				</div>
			{/if}
		</Container>
	</Section>

	<Footer />
</main>
