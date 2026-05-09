<script lang="ts">
	import {
	  Container,
	  Footer,
	  Hero,
	  Reveal,
	  Section,
	  SectionHeader,
	} from "$lib/components";
	import { Button } from "@recast/ui/button";
	import {
	  ArrowRight,
	  Check,
	  Cpu,
	  Download,
	  FileBox,
	  Film,
	  Folder,
	  Gauge,
	  Globe,
	  Layers,
	  Layout,
	  MonitorPlay,
	  MousePointer2,
	  Palette,
	  Play,
	  Scissors,
	  Search,
	  Sparkles,
	  Subtitles,
	  Wand2,
	  WifiOff,
	  Zap,
	} from "lucide-svelte";

	const triad = [
		{
			icon: Film,
			title: "For solo founders",
			description: "Hit record, hit export, hit ship. Investor-ready walkthroughs without a single trip through Final Cut.",
			accent: "from-primary/15 via-transparent to-transparent",
		},
		{
			icon: Cpu,
			title: "For product engineers",
			description: "Demo PRs, explain bugs, and document APIs in clips your team will actually press play on.",
			accent: "from-tertiary/15 via-transparent to-transparent",
		},
		{
			icon: Sparkles,
			title: "For indie hackers",
			description: "Ship launch videos, changelog clips, and Twitter cuts on your own schedule — fully offline.",
			accent: "from-primary/12 via-transparent to-transparent",
		},
	];

	const conversionPoints = [
		"Native capture at full framerate",
		"Real-time cursor smoothing & snap targeting",
		"Auto padding and pixel-perfect framing",
		"Hardware-accelerated H.264 / HEVC export",
		"Re-editable .recast project files",
	];

	const microFeatures = [
		{ icon: Zap, label: "Hardware export", value: "GPU pipeline" },
		{ icon: WifiOff, label: "Local-first", value: "Stays on disk" },
		{ icon: FileBox, label: "Open format", value: ".recast" },
		{ icon: Globe, label: "Cross platform", value: "macOS · Win · Linux" },
	];

	const intelligentFeatures = [
		{
			icon: MousePointer2,
			title: "Cursor refinement",
			description: "Velocity smoothing eliminates twitchy paths and snaps to interactive targets automatically.",
		},
		{
			icon: Layout,
			title: "Auto layouts",
			description: "Padding, backgrounds, and framing applied as you record — no manual setup required.",
		},
		{
			icon: Subtitles,
			title: "Writing assistance",
			description: "Caption, overlay, and chapter scaffolding generated from your recording context.",
		},
		{
			icon: Layers,
			title: "Stack & export",
			description: "Compose multiple takes, swap backgrounds, and ship MP4, WebM, or .recast in seconds.",
		},
	];

	const editorFeatures = [
		{ icon: Scissors, title: "Trim", description: "Frame-accurate ranges" },
		{ icon: Wand2, title: "Refine", description: "Cursor & layout polish" },
		{ icon: Palette, title: "Style", description: "Backgrounds & padding" },
		{ icon: Gauge, title: "Speed ramps", description: "Accelerate boring moments" },
	];
</script>

<svelte:head>
	<title>Recast — Demos that look expensive, without the editor</title>
	<meta
		name="description"
		content="The native screen recorder for founders, indie hackers, and product engineers who'd rather ship than open a timeline. Cinematic out of the box, on macOS, Windows, and Linux."
	/>
</svelte:head>

<main class="text-foreground">
	<Hero />

	<!-- Trust strip -->
	<Section spacing="tight" class="border-t border-border-low/60">
		<Container>
			<Reveal>
				<p class="text-center text-[11px] font-semibold uppercase tracking-[0.2em] text-muted-foreground">
					Built on tools makers trust
				</p>
				<div class="mt-10 flex flex-wrap items-center justify-center gap-x-10 gap-y-7 sm:gap-x-14">
					{#each [
						{ name: "Tauri", slug: "tauri", href: "https://tauri.app" },
						{ name: "Rust", slug: "rust", href: "https://www.rust-lang.org" },
						{ name: "Svelte", slug: "svelte", href: "https://svelte.dev" },
						{ name: "TypeScript", slug: "typescript", href: "https://www.typescriptlang.org" },
						{ name: "Vite", slug: "vite", href: "https://vitejs.dev" },
						{ name: "FFmpeg", slug: "ffmpeg", href: "https://ffmpeg.org" },
						{ name: "Tailwind CSS", slug: "tailwindcss", href: "https://tailwindcss.com" },
						{ name: "GitHub", slug: "github", href: "https://github.com/kanakkholwal/recast" },
					] as logo}
						<a
							href={logo.href}
							target="_blank"
							rel="noopener noreferrer"
							class="group flex items-center gap-2 opacity-50 transition-opacity duration-200 hover:opacity-90"
							title={logo.name}
						>
							<img
								src="https://cdn.simpleicons.org/{logo.slug}/9ca3af"
								alt="{logo.name} logo"
								loading="lazy"
								decoding="async"
								width="20"
								height="20"
								class="h-5 w-5"
							/>
							<span class="text-sm font-semibold tracking-tight text-foreground/55 transition-colors group-hover:text-foreground/85">
								{logo.name}
							</span>
						</a>
					{/each}
				</div>
			</Reveal>
		</Container>
	</Section>

	<!-- Triad cards: Built for modern makers -->
	<Section id="modern" class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Built for modern makers"
				title="A recorder shaped to your workflow."
				description="Whether you're shipping a launch video, an investor walkthrough, or a PR demo — Recast is opinionated where it matters and out of your way everywhere else."
				align="center"
			/>

			<div class="mt-16 grid grid-cols-1 gap-4 md:grid-cols-3">
				{#each triad as item, i}
					{@const Icon = item.icon}
					<Reveal delay={i * 80}>
						<article class="glass-card group relative flex h-full flex-col overflow-hidden rounded-2xl p-6 transition-all duration-300 hover:-translate-y-1 hover:shadow-craft-lg">
							<div class="pointer-events-none absolute inset-0 bg-linear-to-br {item.accent} opacity-60"></div>

							<!-- Mini visual -->
							<div class="relative mb-8 h-44 overflow-hidden rounded-xl border border-border-low/40 bg-background/50">
								<div class="bg-grid bg-grid-fade absolute inset-0 opacity-50"></div>
								<div class="absolute inset-0 flex items-center justify-center">
									<div class="glass-chip flex items-center gap-2 rounded-xl px-3 py-2 shadow-craft-floating">
										<Icon class="size-4 text-primary" />
										<span class="text-xs font-semibold text-foreground">{item.title}</span>
									</div>
								</div>
								<div class="pointer-events-none absolute inset-x-0 bottom-0 h-12 bg-linear-to-t from-background/60 to-transparent"></div>
							</div>

							<div class="relative">
								<h3 class="text-lg font-semibold tracking-tight text-foreground">
									{item.title}
								</h3>
								<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
									{item.description}
								</p>
							</div>
						</article>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Conversion section: two-column with checklist + product preview -->
	<Section id="conversion" class="border-t border-border-low/60">
		<Container>
			<div class="grid items-center gap-14 lg:grid-cols-12 lg:gap-20">
				<div class="lg:col-span-5">
					<SectionHeader
						eyebrow="Recordings that convert"
						title="Demos that earn the play button."
						description="A focused recorder tuned to remove every dead frame between you and a publish-ready demo."
					/>
					<ul class="mt-10 space-y-3.5">
						{#each conversionPoints as point, i}
							<Reveal as="li" delay={i * 60} class="flex items-start gap-3">
								<span class="mt-0.5 grid size-5 shrink-0 place-items-center rounded-md bg-primary/15 text-primary ring-1 ring-primary/20">
									<Check class="size-3.5" />
								</span>
								<span class="text-sm leading-relaxed text-foreground/85">{point}</span>
							</Reveal>
						{/each}
					</ul>
					<div class="mt-10 flex items-center gap-3">
						<Button href="/features" class="gap-2">
							Explore the workflow
							<ArrowRight class="size-4" />
						</Button>
					</div>
				</div>

				<div class="lg:col-span-7">
					<Reveal>
						<div class="glass-card relative overflow-hidden rounded-2xl shadow-craft-xl">
							<div class="flex h-9 items-center gap-2 border-b border-border-low/40 bg-white/3 px-3.5">
								<div class="flex gap-1.5">
									<span class="size-2 rounded-full bg-foreground/15"></span>
									<span class="size-2 rounded-full bg-foreground/15"></span>
									<span class="size-2 rounded-full bg-foreground/15"></span>
								</div>
								<span class="ml-2 text-[10px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
									Recast — Quick Start
								</span>
							</div>
							<div class="bg-linear-to-b from-muted/10 to-background p-1.5">
								<img
									src="/screenshots/preview_homescreen.png"
									alt="Recast quick start"
									loading="lazy"
									decoding="async"
									class="block w-full rounded-xl object-cover ring-1 ring-border-low"
								/>
							</div>

							<div class="glass-chip absolute -left-3 top-12 hidden items-center gap-2 rounded-xl px-3 py-2 shadow-craft-floating sm:flex">
								<MonitorPlay class="size-3.5 text-primary" />
								<span class="text-xs font-semibold text-foreground">Region capture</span>
							</div>
							<div class="glass-chip absolute -right-3 bottom-16 hidden items-center gap-2 rounded-xl px-3 py-2 shadow-craft-floating sm:flex">
								<Wand2 class="size-3.5 text-primary" />
								<span class="text-xs font-semibold text-foreground">Auto-refined</span>
							</div>
						</div>
					</Reveal>
				</div>
			</div>
		</Container>
	</Section>

	<!-- Two side-by-side cards: search + analytics inspired -->
	<Section spacing="tight" class="border-t border-border-low/60">
		<Container>
			<div class="grid gap-4 md:grid-cols-2">
				<Reveal>
					<article class="glass-card flex h-full flex-col gap-6 rounded-2xl p-7">
						<div>
							<h3 class="text-xl font-semibold tracking-tight text-foreground">Capture in two clicks</h3>
							<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
								Region, window, or full screen — start capturing with a single shortcut and zero setup.
							</p>
						</div>

						<!-- Mock search/command bar -->
						<div class="relative rounded-xl border border-border-low/60 bg-background/60 p-4 shadow-craft-inset">
							<div class="flex items-center gap-3 rounded-lg border border-border-low/60 bg-background/80 px-3 py-2.5">
								<Search class="size-4 text-muted-foreground" />
								<span class="text-sm font-medium text-foreground/85">Start a recording…</span>
								<span class="ml-auto rounded-md border border-border-low/60 bg-background px-1.5 py-0.5 font-mono text-[10px] font-semibold text-muted-foreground">⌘ ⇧ R</span>
							</div>
							<div class="mt-3 space-y-1.5">
								{#each [{ icon: MonitorPlay, label: "Record full screen" }, { icon: Layout, label: "Record region" }, { icon: Play, label: "Continue last project" }] as opt, i}
									{@const Icon = opt.icon}
									<div class="flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors {i === 0 ? 'bg-primary/10 text-foreground' : 'text-muted-foreground hover:bg-foreground/5'}">
										<Icon class="size-3.5" />
										<span class="font-medium">{opt.label}</span>
									</div>
								{/each}
							</div>
						</div>
					</article>
				</Reveal>

				<Reveal delay={80}>
					<article class="glass-card flex h-full flex-col gap-6 rounded-2xl p-7">
						<div>
							<h3 class="text-xl font-semibold tracking-tight text-foreground">Refine in real time</h3>
							<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
								Cursor smoothing, padding, and framing applied as the bytes hit disk. No render queue.
							</p>
						</div>

						<!-- Mock activity bars -->
						<div class="rounded-xl border border-border-low/60 bg-background/60 p-5 shadow-craft-inset">
							<div class="flex items-end justify-between gap-2 h-32">
								{#each [40, 65, 35, 80, 55, 90, 70, 45, 78, 95, 60, 82] as h, i}
									<div class="flex h-full flex-1 flex-col justify-end">
										<div
											class="rounded-t-sm bg-linear-to-t from-primary/25 to-primary/80 transition-all"
											style="height: {h}%"
										></div>
									</div>
								{/each}
							</div>
							<div class="mt-4 flex items-center justify-between">
								<div>
									<div class="font-mono text-2xl font-semibold tabular-nums text-foreground">12.4×</div>
									<div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">faster than upload-edit</div>
								</div>
								<div class="text-right">
									<div class="font-mono text-2xl font-semibold tabular-nums text-foreground">0ms</div>
									<div class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">cloud round trips</div>
								</div>
							</div>
						</div>
					</article>
				</Reveal>
			</div>
		</Container>
	</Section>

	<!-- Big editor showcase -->
	<Section id="editor" class="border-t border-border-low/60 overflow-hidden">
		<Container>
			<div class="max-w-2xl">
				<SectionHeader
					eyebrow="The editor"
					title="Trim, tweak, ship."
					description="A focused timeline that respects your time. No nested clip stacks. No floating panels. Just the controls that matter."
				/>
				<div class="mt-8">
					<Button href="/features" variant="outline" class="gap-2">
						Learn more
						<ArrowRight class="size-4" />
					</Button>
				</div>
			</div>

			<div class="relative mt-16">
				<div aria-hidden="true" class="pointer-events-none absolute inset-x-0 -top-20 h-105"
					style="background: radial-gradient(ellipse 70% 60% at 50% 50%, color-mix(in srgb, var(--color-primary) 12%, transparent), transparent 70%);"
				></div>
				<Reveal>
					<div
						class="glass-card relative mx-auto max-w-5xl overflow-hidden rounded-2xl shadow-craft-xl"
						style="transform: perspective(1600px) rotateX(2deg) rotateY(-1deg);"
					>
						<div class="flex h-10 items-center gap-2 border-b border-border-low/40 bg-white/5 px-4">
							<div class="flex gap-1.5">
								<span class="size-2.5 rounded-full bg-foreground/15"></span>
								<span class="size-2.5 rounded-full bg-foreground/15"></span>
								<span class="size-2.5 rounded-full bg-foreground/15"></span>
							</div>
							<span class="ml-3 text-[11px] font-medium text-muted-foreground">Recast — Editor</span>
						</div>
						<div class="bg-linear-to-b from-muted/10 to-background p-1.5">
							<img
								src="/product_preview_hero.png"
								alt="Recast editor"
								loading="lazy"
								decoding="async"
								class="block w-full rounded-xl object-cover ring-1 ring-border-low"
							/>
						</div>
					</div>
				</Reveal>

				<div class="mt-12 grid grid-cols-2 gap-3 md:grid-cols-4">
					{#each editorFeatures as f, i}
						{@const Icon = f.icon}
						<Reveal delay={i * 60}>
							<div class="glass-card flex items-center gap-3 rounded-xl p-4">
								<span class="glass-chip grid size-9 place-items-center rounded-lg text-foreground/70">
									<Icon class="size-4" />
								</span>
								<div>
									<div class="text-sm font-semibold text-foreground">{f.title}</div>
									<div class="text-xs text-muted-foreground">{f.description}</div>
								</div>
							</div>
						</Reveal>
					{/each}
				</div>
			</div>
		</Container>
	</Section>

	<!-- Built for portability: code snippet section -->
	<Section id="format" class="border-t border-border-low/60">
		<Container>
			<div class="grid gap-14 lg:grid-cols-12 lg:gap-20">
				<div class="lg:col-span-5">
					<SectionHeader
						eyebrow="Built for portability"
						title="Your project, in one open file."
						description="The .recast format is a transparent, re-editable artifact. Commit it next to your code or share it across machines — your edits travel with you."
					/>
					<ul class="mt-10 space-y-1">
						{#each [
							{ label: "Project file" },
							{ label: "Re-editable on every platform" },
							{ label: "Versionable and diff-friendly" },
						] as item, i}
							<Reveal as="li" delay={i * 60}>
								<div class="flex items-center gap-3 rounded-lg border-l-2 {i === 0 ? 'border-primary' : 'border-transparent'} pl-4 py-2">
									<span class="text-sm font-medium {i === 0 ? 'text-foreground' : 'text-muted-foreground'}">{item.label}</span>
								</div>
							</Reveal>
						{/each}
					</ul>
				</div>

				<div class="lg:col-span-7">
					<Reveal>
						<div class="glass-card relative overflow-hidden rounded-2xl shadow-craft-lg">
							<div class="flex h-9 items-center gap-3 border-b border-border-low/40 bg-white/3 px-4">
								<div class="flex gap-1.5">
									<span class="size-2 rounded-full bg-foreground/15"></span>
									<span class="size-2 rounded-full bg-foreground/15"></span>
									<span class="size-2 rounded-full bg-foreground/15"></span>
								</div>
								<div class="flex items-center gap-2 rounded-md border border-border-low/60 bg-background/70 px-2 py-0.5">
									<Folder class="size-3 text-muted-foreground" />
									<span class="font-mono text-[10px] font-medium text-foreground/85">demo.recast</span>
								</div>
							</div>
							<pre class="overflow-x-auto p-6 font-mono text-[12.5px] leading-relaxed"><code><span class="text-muted-foreground">{`{`}</span>
  <span class="text-primary">"version"</span><span class="text-muted-foreground">:</span> <span class="text-foreground">"1.0"</span><span class="text-muted-foreground">,</span>
  <span class="text-primary">"name"</span><span class="text-muted-foreground">:</span> <span class="text-foreground">"Quick Start"</span><span class="text-muted-foreground">,</span>
  <span class="text-primary">"capture"</span><span class="text-muted-foreground">: {`{`}</span>
    <span class="text-primary">"source"</span><span class="text-muted-foreground">:</span> <span class="text-foreground">"region"</span><span class="text-muted-foreground">,</span>
    <span class="text-primary">"fps"</span><span class="text-muted-foreground">:</span> <span class="text-foreground">60</span><span class="text-muted-foreground">,</span>
    <span class="text-primary">"display"</span><span class="text-muted-foreground">:</span> <span class="text-foreground">"primary"</span>
  <span class="text-muted-foreground">{`},`}</span>
  <span class="text-primary">"refine"</span><span class="text-muted-foreground">: {`{`}</span>
    <span class="text-primary">"cursor"</span><span class="text-muted-foreground">: {`{ "smooth": true, "snap": true }`}</span><span class="text-muted-foreground">,</span>
    <span class="text-primary">"layout"</span><span class="text-muted-foreground">: {`{ "padding": 64, "background": "aurora" }`}</span>
  <span class="text-muted-foreground">{`},`}</span>
  <span class="text-primary">"export"</span><span class="text-muted-foreground">: {`{`}</span>
    <span class="text-primary">"codec"</span><span class="text-muted-foreground">:</span> <span class="text-foreground">"h264"</span><span class="text-muted-foreground">,</span>
    <span class="text-primary">"hardware"</span><span class="text-muted-foreground">:</span> <span class="text-foreground">true</span>
  <span class="text-muted-foreground">{`}`}</span>
<span class="text-muted-foreground">{`}`}</span></code></pre>
						</div>
					</Reveal>
				</div>
			</div>

			<!-- Micro-feature row -->
			<div class="mt-16 grid grid-cols-2 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 md:grid-cols-4">
				{#each microFeatures as f}
					{@const Icon = f.icon}
					<div class="flex flex-col gap-3 bg-background/50 px-5 py-7 backdrop-blur-md">
						<Icon class="size-4 text-primary" />
						<div>
							<div class="text-sm font-semibold text-foreground">{f.label}</div>
							<div class="mt-1 text-xs text-muted-foreground">{f.value}</div>
						</div>
					</div>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Intelligent features: stacked screenshot card section -->
	<Section id="intelligent" class="border-t border-border-low/60 bg-foreground/1.5 dark:bg-foreground/2">
		<Container>
			<div class="max-w-2xl">
				<SectionHeader
					eyebrow="Refinements that feel automatic"
					title="Polish without the timeline gymnastics."
					description="Smart defaults that turn raw captures into demos worth shipping — no expertise required."
				/>
			</div>

			<!-- Triptych showcase mockup -->
			<Reveal>
				<div class="mt-16 grid gap-4 md:grid-cols-3">
					<!-- Auto layout -->
					<div class="glass-card relative overflow-hidden rounded-2xl p-5 shadow-craft-md">
						<div class="mb-4 flex items-center justify-between">
							<div class="flex items-center gap-2 text-[11px] font-medium text-muted-foreground">
								<Layout class="size-3.5" />
								Auto layout
							</div>
							<span class="rounded-md bg-primary/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-primary">
								Live
							</span>
						</div>
						<div class="space-y-2.5">
							<div class="h-2 w-3/4 rounded-full bg-foreground/10"></div>
							<div class="h-2 w-2/3 rounded-full bg-foreground/10"></div>
							<div class="h-24 rounded-lg bg-linear-to-br from-primary/25 via-primary/10 to-transparent ring-1 ring-primary/20"></div>
							<div class="h-2 w-1/2 rounded-full bg-foreground/10"></div>
						</div>
					</div>

					<!-- Cursor refinement -->
					<div class="glass-card relative overflow-hidden rounded-2xl p-5 shadow-craft-md">
						<div class="mb-4 flex items-center justify-between">
							<div class="flex items-center gap-2 text-[11px] font-medium text-muted-foreground">
								<MousePointer2 class="size-3.5" />
								Cursor refinement
							</div>
							<span class="rounded-md bg-primary/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-primary">
								Auto
							</span>
						</div>
						<div class="relative h-44 overflow-hidden rounded-lg bg-background/50 ring-1 ring-border-low/60">
							<svg class="absolute inset-0 h-full w-full" viewBox="0 0 300 160" preserveAspectRatio="none">
								<path d="M 10 130 Q 60 60 120 80 T 230 60 L 280 40" fill="none" stroke="color-mix(in srgb, var(--color-primary) 70%, transparent)" stroke-width="2" stroke-linecap="round" stroke-dasharray="4 4" />
								<path d="M 10 130 Q 60 60 120 80 T 230 60 L 280 40" fill="none" stroke="var(--color-primary)" stroke-width="2.5" stroke-linecap="round" />
								<circle cx="280" cy="40" r="5" fill="var(--color-primary)" />
								<circle cx="10" cy="130" r="3" fill="color-mix(in srgb, var(--color-foreground) 40%, transparent)" />
							</svg>
						</div>
					</div>

					<!-- Smart polish -->
					<div class="glass-card relative overflow-hidden rounded-2xl p-5 shadow-craft-md">
						<div class="mb-4 flex items-center justify-between">
							<div class="flex items-center gap-2 text-[11px] font-medium text-muted-foreground">
								<Sparkles class="size-3.5 text-primary" />
								Smart polish
							</div>
							<span class="rounded-md bg-primary/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-primary">
								On
							</span>
						</div>
						<div class="space-y-2">
							<div class="flex items-center gap-3 rounded-lg border border-border-low/60 bg-background/60 px-3 py-2">
								<div class="size-2 rounded-full bg-primary"></div>
								<div class="text-sm font-medium text-foreground">Padding</div>
								<div class="ml-auto font-mono text-xs text-muted-foreground">64 px</div>
							</div>
							<div class="flex items-center gap-3 rounded-lg border border-border-low/60 bg-background/60 px-3 py-2">
								<div class="size-2 rounded-full bg-primary"></div>
								<div class="text-sm font-medium text-foreground">Background</div>
								<div class="ml-auto font-mono text-xs text-muted-foreground">aurora</div>
							</div>
							<div class="flex items-center gap-3 rounded-lg border border-border-low/60 bg-background/60 px-3 py-2">
								<div class="size-2 rounded-full bg-primary"></div>
								<div class="text-sm font-medium text-foreground">Cursor</div>
								<div class="ml-auto font-mono text-xs text-muted-foreground">smoothed</div>
							</div>
						</div>
					</div>
				</div>
			</Reveal>

			<div class="mt-16 grid grid-cols-1 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-2 lg:grid-cols-4">
				{#each intelligentFeatures as feature}
					{@const Icon = feature.icon}
					<div class="flex flex-col gap-3 bg-background/50 p-6 backdrop-blur-md">
						<Icon class="size-5 text-primary" />
						<div>
							<div class="text-sm font-semibold text-foreground">{feature.title}</div>
							<div class="mt-1.5 text-xs leading-relaxed text-muted-foreground">{feature.description}</div>
						</div>
					</div>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Final CTA -->
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
							The product demo
							<span class="font-medium italic text-foreground/40">you actually want</span>
							to ship.
						</h2>

						<p class="text-pretty mt-7 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Free during beta. No account required. Three platforms. One opinionated tool.
						</p>

						<div class="mt-10 flex flex-col items-center gap-3 sm:flex-row sm:gap-4">
							<Button href="/download" size="lg" class="gap-2.5">
								<Download class="size-4" />
								Get started
							</Button>
							<Button href="/features" variant="outline" size="lg" class="group/cta gap-2">
								See a demo
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
