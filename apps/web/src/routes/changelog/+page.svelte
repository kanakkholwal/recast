<script lang="ts">
	import {
		Container,
		Eyebrow,
		Footer,
		Reveal,
		Section,
		SectionHeader,
	} from "$lib/components";
	import { Button } from "@recast/ui/button";
	import { Badge } from "@recast/ui/badge";
	import { ArrowUpRight, ExternalLink, GitCommit, Sparkles } from "lucide-svelte";
	import type { PageData } from "./$types";

	let { data }: { data: PageData } = $props();

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

	function renderBody(body: string) {
		const lines = body
			.split(/\r?\n/)
			.map((l) => l.trim())
			.filter(Boolean);

		const items: { type: "heading" | "bullet" | "text"; text: string }[] = [];
		for (const line of lines) {
			if (/^#{1,6}\s/.test(line)) {
				items.push({ type: "heading", text: line.replace(/^#{1,6}\s+/, "") });
			} else if (/^[-*]\s/.test(line)) {
				items.push({ type: "bullet", text: line.replace(/^[-*]\s+/, "") });
			} else {
				items.push({ type: "text", text: line });
			}
		}
		return items;
	}
</script>

<svelte:head>
	<title>Changelog — Recast</title>
	<meta
		name="description"
		content="Every Recast release. Notes, fixes, and what shipped."
	/>
</svelte:head>

<main class="bg-background text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-24">
		<div class="bg-aurora pointer-events-none absolute inset-0 -z-10 opacity-90"></div>
		<div class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 -z-10 opacity-50"></div>

		<Container>
			<div class="mx-auto flex max-w-3xl flex-col items-start gap-7 md:items-center md:text-center">
				<Eyebrow icon={Sparkles} variant="primary">Changelog</Eyebrow>
				<h1 class="text-balance animate-fade-up text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl">
					Every release, in order.
				</h1>
				<p class="text-pretty max-w-xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg" style="animation-delay: 120ms">
					Pulled live from GitHub releases. The latest is at the top.
				</p>
				<div class="flex animate-fade-up gap-3" style="animation-delay: 240ms">
					<Button
						href="https://github.com/kanakkholwal/recast/releases"
						variant="secondary"
						class="gap-2"
					>
						View on GitHub
						<ExternalLink class="size-3.5" />
					</Button>
				</div>
			</div>
		</Container>
	</Section>

	<Section spacing="tight" class="border-t border-border-low/60">
		<Container size="narrow">
			{#if data.releases.length === 0}
				<div class="rounded-2xl border border-border-low bg-card/60 p-10 text-center">
					<h2 class="text-xl font-semibold tracking-tight text-foreground">
						No releases yet
					</h2>
					<p class="mt-2 text-sm text-muted-foreground">
						Recast is in early beta. Check back shortly — or watch the repo on GitHub.
					</p>
				</div>
			{:else}
				<ol class="relative space-y-12 border-l border-border-low/70 pl-6 sm:pl-10">
					{#each data.releases as release, i}
						<Reveal as="li" delay={i * 60} class="relative">
							<span class="absolute -left-[33px] top-1.5 grid size-4 place-items-center rounded-full border border-border-low bg-background sm:-left-[44px]">
								<span class="size-1.5 rounded-full bg-primary"></span>
							</span>
							<article class="rounded-2xl border border-border-low bg-card/70 p-7 transition-all hover:border-border-strong hover:shadow-craft-sm">
								<header class="flex flex-wrap items-center gap-3">
									<h2 class="text-2xl font-semibold tracking-tight text-foreground">
										{release.name}
									</h2>
									{#if release.prerelease}
										<Badge variant="outline" class="border-warning/30 text-warning">
											Pre-release
										</Badge>
									{:else if i === 0}
										<Badge>Latest</Badge>
									{/if}
									<a
										href={release.url}
										target="_blank"
										rel="noopener noreferrer"
										class="ml-auto inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground transition-colors hover:text-foreground"
									>
										View on GitHub
										<ArrowUpRight class="size-3.5" />
									</a>
								</header>

								<div class="mt-2 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
									<span class="inline-flex items-center gap-1.5 font-mono">
										<GitCommit class="size-3.5" />
										{release.tag}
									</span>
									{#if release.publishedAt}
										<span class="text-muted-foreground/40">·</span>
										<time datetime={release.publishedAt}>
											{formatDate(release.publishedAt)}
										</time>
									{/if}
								</div>

								{#if release.body}
									<div class="mt-6 space-y-2 text-sm leading-relaxed text-foreground/80">
										{#each renderBody(release.body) as line}
											{#if line.type === "heading"}
												<h3 class="mt-5 text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
													{line.text}
												</h3>
											{:else if line.type === "bullet"}
												<div class="flex gap-2.5">
													<span class="mt-2 size-1 shrink-0 rounded-full bg-primary/70"></span>
													<span>{line.text}</span>
												</div>
											{:else}
												<p>{line.text}</p>
											{/if}
										{/each}
									</div>
								{:else}
									<p class="mt-6 text-sm italic text-muted-foreground">
										No release notes provided.
									</p>
								{/if}
							</article>
						</Reveal>
					{/each}
				</ol>
			{/if}
		</Container>
	</Section>

	<Footer />
</main>
