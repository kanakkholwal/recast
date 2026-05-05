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
	} from "lucide-svelte";

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
		content="Recast features in depth: cursor engine, smart layouts, lightweight editor, GPU export, and more."
	/>
</svelte:head>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-20 md:pt-48 md:pb-24">
		<Container>
			<div class="mx-auto flex max-w-3xl flex-col items-start gap-7 text-left md:items-center md:text-center">
				<Eyebrow icon={Sparkles} variant="primary">Features</Eyebrow>
				<h1 class="text-balance animate-fade-up text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl">
					Everything Recast
					<span class="inline-grid overflow-hidden align-bottom font-medium italic text-foreground/40">
						<TextLoop class="text-primary" texts={verbs} interval={2800} />
					</span>
				</h1>
				<p class="text-pretty max-w-2xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg" style="animation-delay: 120ms">
					A focused recorder, not a kitchen-sink editor. Three pillars do most of the work; the rest stays out of your way.
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

	<Section spacing="tight" class="border-t border-border-low/60">
		<Container>
			<Reveal>
				<div class="glass-card relative overflow-hidden rounded-3xl px-6 py-14 text-center shadow-craft-lg sm:px-12 sm:py-20">
					<div class="bg-aurora pointer-events-none absolute inset-0 opacity-60"></div>
					<div class="relative mx-auto flex max-w-2xl flex-col items-center gap-6">
						<h2 class="text-balance text-3xl font-semibold tracking-tight text-foreground sm:text-4xl md:text-5xl">
							Skip the editor. Ship the demo.
						</h2>
						<div class="flex flex-col gap-3 sm:flex-row sm:gap-4">
							<Button href="/download" size="lg" class="gap-2.5">
								<Download class="size-4" />
								Download Recast
							</Button>
							<Button href="/changelog" variant="ghost" size="lg" class="gap-2 group/cta">
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
