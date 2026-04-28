<script lang="ts">
	import {
		Container,
		Eyebrow,
		FeatureGrid,
		Footer,
		Hero,
		Reveal,
		Section,
		SectionHeader,
	} from "$lib/components";
	import { Button } from "@recast/ui/button";
	import {
		ArrowRight,
		Download,
		Play,
		Rocket,
		Terminal,
		Users,
		Wand2,
	} from "lucide-svelte";

	const workflow = [
		{
			step: "01",
			icon: Play,
			title: "Capture",
			description:
				"Native screen capture at full framerate. Window, region, or full screen — with negligible overhead.",
		},
		{
			step: "02",
			icon: Wand2,
			title: "Refine",
			description:
				"Cursor smoothing, smart padding, and clean backgrounds applied in real time. No timeline required.",
		},
		{
			step: "03",
			icon: Download,
			title: "Ship",
			description:
				"Hardware-accelerated export in seconds. Or save as .recast and re-edit any time, anywhere.",
		},
	];

	const useCases = [
		{
			icon: Terminal,
			title: "Engineers",
			description:
				"Demo PRs, explain bugs, document APIs. Without writing a 2,000-word post.",
		},
		{
			icon: Rocket,
			title: "Founders",
			description:
				"Pixel-perfect product walkthroughs that actually feel premium to investors and users.",
		},
		{
			icon: Users,
			title: "Teams",
			description:
				"Replace asynchronous chaos with crisp, watchable updates that get watched.",
		},
	];
</script>

<svelte:head>
	<title>Recast — The intentional screen recorder</title>
	<meta
		name="description"
		content="Record, refine, and ship cinematic product walkthroughs — locally, in seconds. Free during beta. macOS, Windows, Linux."
	/>
</svelte:head>

<main class="bg-background text-foreground">
	<Hero />

	<Section id="workflow" class="border-t border-border-low/60">
		<Container>
			<div class="grid gap-16 lg:grid-cols-12 lg:gap-20">
				<div class="lg:col-span-5 lg:sticky lg:top-32 lg:self-start">
					<SectionHeader
						eyebrow="Crafted workflow"
						title="Three steps. Zero friction."
						description="Recast collapses the demo workflow to its essentials — capture, polish, ship — and removes the dead weight in between."
					>
						{#snippet actions()}
							<Button href="/features" variant="ghost" class="gap-2">
								See it end-to-end
								<ArrowRight class="size-4" />
							</Button>
						{/snippet}
					</SectionHeader>
				</div>

				<ol class="lg:col-span-7 space-y-4">
					{#each workflow as item, i}
						{@const Icon = item.icon}
						<Reveal as="li" delay={i * 100}>
							<article
								class="group relative flex gap-5 rounded-2xl border border-border-low bg-card/60 p-6 transition-all duration-300 hover:border-border-strong hover:bg-card hover:shadow-craft-md sm:p-8"
							>
								<div class="flex shrink-0 flex-col items-center gap-3">
									<span
										class="grid size-12 place-items-center rounded-xl border border-border-low bg-background text-foreground/70 transition-colors group-hover:bg-primary/10 group-hover:text-primary"
									>
										<Icon class="size-5" />
									</span>
									{#if i < workflow.length - 1}
										<span class="hidden h-full w-px bg-border-low sm:block"></span>
									{/if}
								</div>
								<div class="flex-1">
									<span class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
										Step {item.step}
									</span>
									<h3 class="mt-2 text-2xl font-semibold tracking-tight text-foreground">
										{item.title}
									</h3>
									<p class="mt-2.5 text-pretty text-base leading-relaxed text-muted-foreground">
										{item.description}
									</p>
								</div>
							</article>
						</Reveal>
					{/each}
				</ol>
			</div>
		</Container>
	</Section>

	<FeatureGrid />

	<Section id="use-cases" class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Purpose-built"
				title="Made for people who ship."
				description="Not a bloated editor. Not a laggy cloud recorder. Built for high-fidelity communication."
				align="center"
			/>

			<div class="mt-16 grid grid-cols-1 gap-4 md:grid-cols-3">
				{#each useCases as useCase, i}
					{@const Icon = useCase.icon}
					<Reveal delay={i * 80}>
						<article
							class="group relative flex h-full flex-col overflow-hidden rounded-2xl border border-border-low bg-card/60 p-7 transition-all duration-300 hover:-translate-y-1 hover:border-border-strong hover:shadow-craft-md"
						>
							<div
								class="pointer-events-none absolute -right-12 -top-12 size-32 rounded-full bg-primary/5 blur-3xl transition-colors duration-500 group-hover:bg-primary/12"
							></div>
							<span
								class="grid size-11 place-items-center rounded-xl border border-border-low bg-background text-foreground/70 transition-all duration-300 group-hover:scale-105 group-hover:bg-primary/10 group-hover:text-primary"
							>
								<Icon class="size-5" />
							</span>
							<h3 class="mt-8 text-xl font-semibold tracking-tight text-foreground">
								{useCase.title}
							</h3>
							<p class="mt-2.5 text-pretty text-sm leading-relaxed text-muted-foreground">
								{useCase.description}
							</p>
						</article>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<Section id="cta" class="border-t border-border-low/60">
		<Container>
			<Reveal>
				<div
					class="relative overflow-hidden rounded-3xl border border-border-low bg-card/80 px-6 py-16 text-center shadow-craft-lg sm:px-12 sm:py-20 md:py-24"
				>
					<div class="bg-aurora pointer-events-none absolute inset-0 opacity-80"></div>
					<div class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 opacity-50"></div>

					<div class="relative mx-auto flex max-w-2xl flex-col items-center gap-7">
						<Eyebrow variant="primary">Ready when you are</Eyebrow>
						<h2 class="text-balance text-4xl font-semibold tracking-tight text-foreground sm:text-5xl md:text-6xl">
							Refine your next product demo.
						</h2>
						<p class="text-pretty text-base text-muted-foreground sm:text-lg">
							Free during beta. No account required. Three platforms. One opinionated tool.
						</p>
						<div class="mt-2 flex flex-col items-center gap-3 sm:flex-row sm:gap-4">
							<Button href="/download" size="lg" class="gap-2.5">
								<Download class="size-4" />
								Download Recast
							</Button>
							<Button href="/features" variant="ghost" size="lg" class="group/cta gap-2">
								Explore features
								<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
							</Button>
						</div>
					</div>
				</div>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
