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
	import { TextLoop } from "$lib/motion-core";
	import {
		ArrowRight,
		Cpu,
		Crop,
		Download,
		FileBox,
		Keyboard,
		Layout,
		MousePointer2,
		Palette,
		Scissors,
		ShieldCheck,
		Sparkles,
		WifiOff,
		Zap,
	} from "@lucide/svelte";

	const pillars = [
		{
			icon: MousePointer2,
			title: "Cursor engine",
			description:
				"Velocity-based smoothing, snap targeting, and motion damping. Your cursor stops feeling like a tracked mouse and starts feeling like a director's hand.",
			tags: ["Smoothing", "Snap targeting", "Trail decay"],
		},
		{
			icon: Layout,
			title: "Smart layouts",
			description:
				"Auto padding, gradient backgrounds, and aspect framing applied as you record. Drop in a recording, get a polished frame.",
			tags: ["Auto padding", "Backgrounds", "16:9 / 4:3 / 9:16"],
		},
		{
			icon: Scissors,
			title: "Lightweight editor",
			description:
				"Trim, split, replace background. A focused editor that respects your time — no hidden timeline tax.",
			tags: ["Trim & split", "Replace bg", "Speed ramps"],
		},
	];

	const supports = [
		{
			icon: Zap,
			title: "GPU-accelerated export",
			description: "Hardware-encoded H.264 / HEVC pipelines. Seconds, not minutes.",
		},
		{
			icon: WifiOff,
			title: "Offline first",
			description:
				"Recordings, projects, and exports never leave your machine. No accounts.",
		},
		{
			icon: FileBox,
			title: "Project files (.recast)",
			description:
				"Re-editable artifacts that travel with your repo. Time-travel through cuts.",
		},
		{
			icon: Cpu,
			title: "Native capture",
			description:
				"Built on platform APIs. Negligible overhead, full framerate, no proxy layers.",
		},
		{
			icon: Crop,
			title: "Region & window",
			description:
				"Capture a window, region, or full screen. Hot-swap mid-take.",
		},
		{
			icon: Palette,
			title: "Theme-aware",
			description:
				"Light and dark presets that match your OS, not someone else's brand.",
		},
		{
			icon: Keyboard,
			title: "Shortcut-first",
			description:
				"Every essential action lives one keystroke away. Mouse optional.",
		},
		{
			icon: ShieldCheck,
			title: "Open source",
			description:
				"MIT-licensed. Audit it, fork it, ship it. Source on GitHub.",
		},
	];

	const verbs = ["records.", "refines.", "ships.", "exports.", "delights."];
</script>

<svelte:head>
	<title>Features — Recast</title>
	<meta
		name="description"
		content="The cursor engine, smart layouts, and native capture that auto-polish raw screen captures into shareable demos — built for solo founders."
	/>
</svelte:head>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-20 md:pt-48 md:pb-24">
		<Container>
			<div class="mx-auto flex max-w-3xl flex-col items-start gap-7 text-left md:items-center md:text-center">
				<Eyebrow icon={Sparkles} variant="primary">Features</Eyebrow>
				<h1 class="text-balance animate-fade-up text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl">
					Everything Recast
					<span class="mt-2 flex justify-start font-medium italic text-foreground/40 md:justify-center">
						<span class="inline-grid overflow-hidden">
							<TextLoop class="text-primary" texts={verbs} interval={2800} />
						</span>
					</span>
				</h1>
				<p class="text-pretty max-w-2xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg" style="animation-delay: 120ms">
					Record, auto-polish, share. A focused recorder — not a kitchen-sink editor — that turns raw captures into demos worth shipping.
				</p>
			</div>
		</Container>
	</Section>

	<Section spacing="tight" class="border-t border-border-low/60">
		<Container>
			<div class="grid gap-4 lg:grid-cols-3">
				{#each pillars as pillar, i}
					{@const Icon = pillar.icon}
					<Reveal delay={i * 80}>
						<article class="glass-card group relative flex h-full flex-col overflow-hidden rounded-2xl p-8 transition-all duration-300 hover:-translate-y-1 hover:shadow-craft-md">
							<div class="pointer-events-none absolute -right-12 -top-12 size-40 rounded-full bg-primary/8 blur-3xl transition-opacity duration-500 group-hover:bg-primary/14"></div>
							<span class="glass-chip grid size-12 place-items-center rounded-xl text-foreground/70 transition-all group-hover:scale-105 group-hover:text-primary">
								<Icon class="size-5" />
							</span>
							<h3 class="mt-6 text-xl font-semibold tracking-tight text-foreground">
								{pillar.title}
							</h3>
							<p class="mt-2.5 text-pretty text-sm leading-relaxed text-muted-foreground">
								{pillar.description}
							</p>
							<ul class="mt-6 flex flex-wrap gap-2">
								{#each pillar.tags as tag}
									<li class="glass-chip rounded-full px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
										{tag}
									</li>
								{/each}
							</ul>
						</article>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<Section class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Built in"
				title="The supporting cast."
				description="The smaller details that make Recast feel like the right tool every time."
			/>

			<div class="mt-14 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
				{#each supports as item, i}
					{@const Icon = item.icon}
					<Reveal delay={i * 50}>
						<article class="glass-card group h-full rounded-xl p-5 transition-all duration-300 hover:-translate-y-0.5 hover:shadow-craft-sm">
							<span class="glass-chip grid size-9 place-items-center rounded-lg text-foreground/70 transition-colors group-hover:text-primary">
								<Icon class="size-4" />
							</span>
							<h4 class="mt-5 text-sm font-semibold tracking-tight text-foreground">
								{item.title}
							</h4>
							<p class="mt-1.5 text-pretty text-xs leading-relaxed text-muted-foreground">
								{item.description}
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
						class="pointer-events-none absolute -top-40 left-1/2 size-160 -translate-x-1/2 rounded-full opacity-60"
						style="background: radial-gradient(closest-side, color-mix(in srgb, var(--color-primary) 22%, transparent), transparent 70%);"
					></div>
					<div
						aria-hidden="true"
						class="pointer-events-none absolute inset-x-0 top-0 h-px"
						style="background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--color-foreground) 18%, transparent), transparent);"
					></div>

					<div class="relative mx-auto flex max-w-3xl flex-col items-center text-center">
						<div class="glass-chip inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/80">
							<span class="relative flex size-1.5">
								<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-70"></span>
								<span class="relative inline-flex size-1.5 rounded-full bg-primary"></span>
							</span>
							v0.1 beta · ready when you are
						</div>

						<h2 class="text-balance mt-8 text-4xl font-semibold leading-[1.02] tracking-tight text-foreground sm:text-5xl md:text-6xl lg:text-[4.25rem]">
							Skip the editor.
							<span class="block font-medium italic text-foreground/40">Ship the demo.</span>
						</h2>

						<p class="text-pretty mt-7 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Free forever. No account required. Three platforms. One opinionated tool.
						</p>

						<div class="mt-10 flex flex-col items-center gap-3 sm:flex-row sm:gap-4">
							<Button href="/download" size="lg" class="gap-2.5">
								<Download class="size-4" />
								Download Recast
							</Button>
							<Button href="/changelog" variant="outline" size="lg" class="group/cta gap-2">
								See what's new
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
