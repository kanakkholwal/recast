<script lang="ts">
	import {
	  Container,
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
	  FileBox,
	  Play,
	  Rocket,
	  Terminal,
	  Users,
	  Wand2,
	  WifiOff,
	  Zap,
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
	<title>Recast - The intentional screen recorder</title>
	<meta
		name="description"
		content="Record, refine, and ship cinematic product walkthroughs — locally, in seconds. Free during beta. macOS, Windows, Linux."
	/>
</svelte:head>

<main class="text-foreground">
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
								class="glass-card group relative flex gap-5 rounded-2xl p-6 transition-all duration-300 hover:-translate-y-0.5 hover:shadow-craft-md sm:p-8"
							>
								<div class="flex shrink-0 flex-col items-center gap-3">
									<span
										class="glass-chip grid size-12 place-items-center rounded-xl text-foreground/70 transition-colors group-hover:text-primary"
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
							class="glass-card group relative flex h-full flex-col overflow-hidden rounded-2xl p-7 transition-all duration-300 hover:-translate-y-1 hover:shadow-craft-md"
						>
							<div
								class="pointer-events-none absolute -right-12 -top-12 size-32 rounded-full bg-primary/5 blur-3xl transition-colors duration-500 group-hover:bg-primary/12"
							></div>
							<span
								class="glass-chip grid size-11 place-items-center rounded-xl text-foreground/70 transition-all duration-300 group-hover:scale-105 group-hover:text-primary"
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
					class="glass-card relative overflow-hidden rounded-[2rem] px-6 py-16 sm:px-14 sm:py-20 md:py-24"
					style="box-shadow: inset 0 1px 0 0 color-mix(in srgb, white 12%, transparent), inset 0 -1px 0 0 color-mix(in srgb, var(--color-foreground) 4%, transparent);"
				>
					<div
						aria-hidden="true"
						class="pointer-events-none absolute -top-40 left-1/2 size-[640px] -translate-x-1/2 rounded-full opacity-60"
						style="background: radial-gradient(closest-side, color-mix(in srgb, var(--color-primary) 18%, transparent), transparent 70%);"
					></div>
					<div
						aria-hidden="true"
						class="pointer-events-none absolute inset-x-0 top-0 h-px"
						style="background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--color-foreground) 18%, transparent), transparent);"
					></div>

					<div class="relative mx-auto flex max-w-3xl flex-col items-center text-center">
						<div
							class="glass-chip inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/80"
						>
							<span class="relative flex size-1.5">
								<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-70"></span>
								<span class="relative inline-flex size-1.5 rounded-full bg-primary"></span>
							</span>
							v0.1 beta · ready when you are
						</div>

						<h2
							class="text-balance mt-8 text-4xl font-semibold leading-[1.02] tracking-tight text-foreground sm:text-5xl md:text-6xl lg:text-[4.25rem]"
						>
							Refine your next
							<span class="font-medium italic text-foreground/40">product demo.</span>
						</h2>

						<p
							class="text-pretty mt-7 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg"
						>
							Free during beta. No account required. Three platforms. One opinionated tool.
						</p>

						<div class="mt-10 flex flex-col items-center gap-3 sm:flex-row sm:gap-4">
							<Button href="/download" size="lg" class="gap-2.5">
								<Download class="size-4" />
								Download Recast
							</Button>
							<Button href="/features" variant="ghost" size="lg" class="group/cta gap-2">
								Explore features
								<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
							</Button>
						</div>

						<div
							class="mt-14 grid w-full max-w-2xl grid-cols-3 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30"
						>
							{#each [{ icon: WifiOff, label: "Offline-first", value: "100%" }, { icon: Zap, label: "Export speed", value: "GPU" }, { icon: FileBox, label: "Project files", value: ".recast" }] as stat}
								{@const Icon = stat.icon}
								<div
									class="flex flex-col items-center gap-1.5 bg-background/40 px-4 py-5 backdrop-blur-md"
								>
									<span class="flex items-center gap-1.5 text-muted-foreground">
										<Icon class="size-3.5" />
										<span class="text-[10px] font-semibold uppercase tracking-[0.16em]">
											{stat.label}
										</span>
									</span>
									<span class="font-mono text-lg font-semibold text-foreground">
										{stat.value}
									</span>
								</div>
							{/each}
						</div>
					</div>
				</div>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
