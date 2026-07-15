<script lang="ts">
	import {
	  Container,
	  Eyebrow,
	  Footer,
	  HeroBackdrop,
	  Reveal,
	  Section,
	  SectionHeader,
	  SeoMeta,
	} from "$lib/components";
	import { TextLoop } from "$lib/motion-core";
	import {
	  Apple,
	  ArrowRight,
	  Camera,
	  Check,
	  Cpu,
	  Crop,
	  FileBox,
	  HardDrive,
	  HardDriveUpload,
	  Highlighter,
	  Keyboard,
	  Layers,
	  Layout,
	  Monitor,
	  MousePointer2,
	  Pause,
	  Scissors,
	  Target,
	  Terminal,
	  VolumeX,
	  Wand2,
	  WifiOff,
	  Zap
	} from "@lucide/svelte";
	import { GithubBrand } from "@recast/ui/brand-icons";
	import { Button } from "@recast/ui/button";
	import { cn } from "@recast/ui/utils";

	// Three pillars chosen to lead with what makes Recast different, not
	// generic "we have an editor too" copy. Each one is a feature that other
	// recorders either don't have at all or paywall.
	const pillars = [
		{
			icon: Wand2,
			title: "Auto-polish on the way in",
			description:
				"Smart zoom, cursor smoothing, and silence cuts happen while you record. By the time you stop, the demo is mostly done. No keyframes. No AI gate.",
			tags: ["Smart zoom", "Cursor smoothing", "Silence cuts"],
		},
		{
			icon: Layers,
			title: "Recording profiles + pause / resume",
			description:
				"Save capture presets (region, window, camera, mic) and switch with one shortcut. Pause mid-take when the door knocks. Paused spans are trimmed cleanly out.",
			tags: ["Profiles", "Pause and resume", "Multi-source"],
		},
		{
			icon: HardDriveUpload,
			title: "Local-first, Drive-shareable",
			description:
				"Recordings live on your machine until you choose to share. Upload exports straight to your own Google Drive from the export dialog. You own the file, Recast servers never see it.",
			tags: ["Local files", "Drive upload", "Own your data"],
		},
	];

	// Per-platform shipping confidence. Mirrors the /download page so the
	// marketing voice never over-promises macOS or Linux.
	const platforms = [
		{ icon: Monitor, label: "Windows", stability: "stable" as const, note: "Daily-driver stable" },
		{ icon: Apple, label: "macOS", stability: "beta" as const, note: "Active beta (12.0+)" },
		{ icon: Terminal, label: "Linux", stability: "beta" as const, note: "Active beta (Wayland + X11)" },
	];

	const stabilityChip: Record<"stable" | "beta", { label: string; cls: string }> = {
		stable: {
			label: "Stable",
			cls: "bg-emerald-500/12 text-emerald-600 ring-emerald-500/25 dark:text-emerald-400",
		},
		beta: {
			label: "Beta",
			cls: "bg-amber-500/12 text-amber-600 ring-amber-500/25 dark:text-amber-400",
		},
	};

	// "Free here, paid elsewhere." Real, named feature gaps against the two
	// products we get compared to most. Conservative claims: only items that
	// are publicly paywalled on the competitor or that the competitor doesn't
	// ship at all. Tone is direct, not snide.
	const gapRows = [
		{
			feature: "Recording profiles (capture presets)",
			recast: "Built in",
			loom: "Not available",
			cap: "Limited",
		},
		{
			feature: "Hardware-accelerated export",
			recast: "Built in",
			loom: "Cloud render only",
			cap: "Partial",
		},
		{
			feature: "Files stay on your machine",
			recast: "Default",
			loom: "Cloud only",
			cap: "Local first",
		},
		{
			feature: "Share to your own storage (Drive today)",
			recast: "Built in, free",
			loom: "Not supported",
			cap: "Pro only",
		},
		{
			feature: "No account required to record",
			recast: "Never asks",
			loom: "Required",
			cap: "Required",
		},
		{
			feature: "Open source",
			recast: "GPLv3",
			loom: "Closed",
			cap: "AGPL",
		},
		{
			feature: "Per-seat pricing",
			recast: "None",
			loom: "Per seat",
			cap: "Per seat",
		},
	];

	// Built-in supports. Grid of small affordances and standards-level
	// features. License row is GPLv3 + dual licensing (not MIT, which was
	// the previous version's bug).
	//
	// `tag` is the small module-name badge that sits in the screenshot
	// corner of each vendor card (e.g. "Capture", "Edit", "Export").
	// `image` is a real screenshot when one exists; until then the card
	// renders a tinted icon-as-hero placeholder the same width/height,
	// so the layout is stable once real images drop in.
	const supports: Array<{
		icon: typeof Target;
		tag: string;
		title: string;
		description: string;
		image: string | null;
		href: string;
	}> = [
		{
			icon: Target,
			tag: "Auto",
			title: "Smart auto-zoom",
			description: "Reads clicks and dwell, zooms toward the action. Zero keyframes.",
			image: null,
			href: "#",
		},
		{
			icon: MousePointer2,
			tag: "Cursor",
			title: "Cursor smoothing",
			description: "Velocity-aware easing, optional snap-to-target, motion damping.",
			image: null,
			href: "#",
		},
		{
			icon: VolumeX,
			tag: "Audio",
			title: "Silence detection",
			description: "Finds dead-air spans (quiet audio plus still cursor), offers one-click cuts.",
			image: null,
			href: "#",
		},
		{
			icon: Pause,
			tag: "Capture",
			title: "Pause and resume",
			description: "Pause mid-take and pick up where you left off. Paused spans trim out cleanly.",
			image: null,
			href: "#",
		},
		{
			icon: Layers,
			tag: "Capture",
			title: "Recording profiles",
			description: "Save capture presets for each context. One shortcut to switch between them.",
			image: null,
			href: "#",
		},
		{
			icon: Highlighter,
			tag: "Edit",
			title: "Annotations and blur",
			description: "Arrows, rectangles, text, privacy blur on the frame. Layers on the timeline.",
			image: null,
			href: "#",
		},
		{
			icon: Camera,
			tag: "Capture",
			title: "Camera bubble",
			description: "Draggable webcam with shape, border, and follow-the-cursor motion.",
			image: null,
			href: "#",
		},
		{
			icon: Layout,
			tag: "Layout",
			title: "Smart layouts",
			description: "Auto padding, gradient backgrounds, aspect framing applied as you record.",
			image: null,
			href: "#",
		},
		{
			icon: Scissors,
			tag: "Edit",
			title: "Trim, split, replace",
			description: "Lightweight editor that respects your time. No hidden timeline tax.",
			image: null,
			href: "#",
		},
		{
			icon: HardDriveUpload,
			tag: "Store",
			title: "Drive uploads",
			description: "OAuth scoped to files Recast creates. Your account, your storage bill.",
			image: null,
			href: "#",
		},
		{
			icon: Zap,
			tag: "Export",
			title: "Hardware-encoded export",
			description: "NVENC, AMD, and Intel where available. Seconds, not minutes.",
			image: null,
			href: "#",
		},
		{
			icon: Cpu,
			tag: "Capture",
			title: "Native capture",
			description: "Platform APIs end to end. ScreenCaptureKit on macOS, Wayland-native on Linux.",
			image: null,
			href: "#",
		},
		{
			icon: Crop,
			tag: "Capture",
			title: "Region and window",
			description: "Capture a window, region, or full screen. Hot-swap mid-take.",
			image: null,
			href: "#",
		},
		{
			icon: FileBox,
			tag: "Files",
			title: ".recast project files",
			description: "Re-editable artifacts that travel with your repo.",
			image: null,
			href: "#",
		},
		{
			icon: WifiOff,
			tag: "Offline",
			title: "Offline first",
			description: "Recordings and exports stay on your machine. No account required to record.",
			image: null,
			href: "#",
		},
		{
			icon: HardDrive,
			tag: "Privacy",
			title: "No telemetry",
			description: "The app doesn't phone home. It only contacts servers when you explicitly opt in.",
			image: null,
			href: "#",
		},
		{
			icon: Keyboard,
			tag: "UX",
			title: "Shortcut-first",
			description: "Every essential action lives one keystroke away. Mouse optional.",
			image: null,
			href: "#",
		},
		{
			icon: GithubBrand,
			tag: "OSS",
			title: "GPLv3 open source",
			description: "Source on GitHub. Dual licensing available for closed-source redistribution.",
			image: null,
			href: "#",
		},
	];

	const verbs = ["records.", "polishes.", "shares.", "ships."];
</script>

<SeoMeta
	title="Everything Recast does for you"
	description="Recording profiles, pause and resume, smart auto-zoom, cursor smoothing, silence cuts, on-frame annotations, Drive sharing. The full feature catalog."
	eyebrow="Features"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-20 md:pt-48 md:pb-24">
		<HeroBackdrop src="/background-features.webp" tone="strong" />
		<Container class="relative">
			<div class="relative z-10 mx-auto flex max-w-3xl flex-col items-center gap-7 text-center">
				<span class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70">
					<span class="size-1.5 rounded-full bg-primary"></span>
					Features
				</span>
				<h1 class="text-balance text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5rem]">
					Everything Recast
					<span class="mt-2 flex justify-center font-medium italic text-foreground/40">
						<span class="inline-grid overflow-hidden">
							<TextLoop class="text-primary" texts={verbs} interval={2800} />
						</span>
					</span>
				</h1>
				<p class="text-pretty max-w-2xl text-base leading-relaxed text-muted-foreground sm:text-lg">
					A focused recorder for solo founders, indie hackers, and product engineers who'd rather ship than fiddle. Auto-polish for the 80% case, a minimal timeline for the moments you want to control.
				</p>

				<!-- Platform chips: honest about per-platform maturity. -->
				<ul class="mt-2 flex flex-wrap items-center justify-center gap-2 text-[11.5px] font-semibold">
					{#each platforms as p (p.label)}
						{@const Icon = p.icon}
						{@const chip = stabilityChip[p.stability]}
						<li class="inline-flex items-center gap-2 rounded-full border border-border-low/50 bg-card/40 px-3 py-1.5 text-foreground/85 ring-1 ring-inset ring-border-low/30">
							<Icon class="size-3.5" />
							{p.label}
							<span class={cn("ml-0.5 inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[9.5px] font-bold uppercase tracking-[0.14em] ring-1 ring-inset", chip.cls)}>
								{chip.label}
							</span>
						</li>
					{/each}
				</ul>
			</div>
		</Container>
	</Section>

	<!-- Three pillars: lead with differentiators. -->
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

	<!-- "Free here, paid elsewhere." Concrete value-gap table. Compares
	     against the two products we get compared to most, with conservative
	     claims and a direct tone. -->
	<Section class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Side by side"
				title="Free here. Paid in the others."
				description="Most of what Recast ships in the free desktop app is either paywalled or missing in the products we get compared to most. The honest version."
				align="center"
			/>

			<Reveal variant="blur" class="mt-14">
				<div class="overflow-x-auto rounded-2xl border border-border-low/50">
					<div class="min-w-[640px]">
						<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr] border-b border-border-low/50 bg-foreground/2 text-[11px] font-semibold uppercase tracking-[0.16em]">
							<div class="px-5 py-3.5 text-muted-foreground">Feature</div>
							<div class="border-l border-border-low/50 px-5 py-3.5 text-center text-primary">Recast</div>
							<div class="border-l border-border-low/50 px-5 py-3.5 text-center text-foreground/80">Loom</div>
							<div class="border-l border-border-low/50 px-5 py-3.5 text-center text-foreground/80">Cap</div>
						</div>
						{#each gapRows as row, i}
							<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr] {i < gapRows.length - 1 ? 'border-b border-border-low/40' : ''}">
								<div class="px-5 py-3.5 text-sm text-foreground/85">{row.feature}</div>
								<div class="flex items-center justify-center border-l border-border-low/40 bg-primary/4 px-5 py-3.5 text-center">
									<span class="inline-flex items-center gap-1.5 text-xs font-semibold text-foreground">
										<Check class="size-3.5 text-primary" />
										{row.recast}
									</span>
								</div>
								<div class="flex items-center justify-center border-l border-border-low/40 px-5 py-3.5 text-center text-xs text-muted-foreground">
									{row.loom}
								</div>
								<div class="flex items-center justify-center border-l border-border-low/40 px-5 py-3.5 text-center text-xs text-muted-foreground">
									{row.cap}
								</div>
							</div>
						{/each}
					</div>
				</div>
			</Reveal>

			<Reveal variant="up" class="mt-6">
				<p class="mx-auto max-w-2xl text-balance text-center text-xs leading-relaxed text-muted-foreground">
					Comparison is based on the publicly documented tiers of each product. Got a correction? Open an issue on
					<a href="https://github.com/kanakkholwal/recast" class="text-foreground underline-offset-2 hover:underline">GitHub</a>.
				</p>
			</Reveal>
		</Container>
	</Section>

	<!--
	  Full catalog, vendor layout. Every third card (1, 4, 7, 10, 13, 16)
	  spans 2 cols for visual rhythm and to highlight the marquee features
	  (auto-zoom, pause-and-resume, hardware export, native capture,
	  offline-first, open-source). Each card renders its `image` when one
	  exists, with the feature icon as a tinted placeholder until then. The
	  small "tag" badge sits in the screenshot's bottom-right corner (the
	  module name: Capture, Edit, Export, Privacy…). On lg the big card
	  lays out horizontally (image left, text right); normal cards stack
	  vertically. On mobile every card drops to 1 col.
	-->
	<Section class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Built in"
				title="The full catalog."
				description="Every affordance worth naming, in one grid. All shipping in the free desktop app today."
			/>

			<div class="mt-14 grid auto-rows-fr grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
				{#each supports as item, i}
					{@const Icon = item.icon}
					{@const isFeatured = i % 3 === 0}
					<Reveal delay={i * 35} class={isFeatured ? "lg:col-span-2" : ""}>
						<article
							class={cn(
								"glass-card group relative flex h-full overflow-hidden rounded-2xl border border-border-low/40 bg-card/40 transition-[transform,box-shadow,border-color] duration-300 hover:-translate-y-0.5 hover:border-border-low hover:shadow-craft-sm motion-reduce:transition-none",
								isFeatured
									? "flex-col p-0 lg:flex-row lg:items-stretch"
									: "flex-col p-5",
							)}
						>
							<!--
							  Visual panel. Real screenshot when one exists, otherwise a
							  tinted icon-as-hero placeholder the same dimensions so the
							  card stays stable once a real image drops in. The "tag" badge
							  anchors to the bottom-right of the visual.
							-->
							<div
								class={cn(
									"relative overflow-hidden",
									isFeatured
										? "aspect-[4/3] shrink-0 border-b border-border-low/40 lg:aspect-auto lg:w-1/2 lg:border-b-0 lg:border-r"
										: "aspect-[16/10] w-full",
								)}
							>
								{#if item.image}
									<img
										src={item.image}
										alt={item.title}
										loading="lazy"
										decoding="async"
										class="absolute inset-0 size-full object-cover"
									/>
								{:else}
									<!-- Tinted placeholder. Soft radial wash from the top so the
									     placeholder reads as a "screenshot slot" rather than a
									     missing image. -->
									<div
										class="absolute inset-0"
										style="background: linear-gradient(160deg, color-mix(in srgb, var(--color-foreground) 7%, transparent) 0%, color-mix(in srgb, var(--color-foreground) 3%, transparent) 60%, transparent 100%);"
									></div>
									<div
										class="absolute inset-0 grid place-items-center"
									>
										<div
											class={cn(
												"grid place-items-center rounded-2xl border border-border-low/40 bg-card/60 shadow-craft-sm",
												isFeatured ? "size-20" : "size-14",
											)}
										>
											<Icon
												class={cn(
													"text-foreground/75",
													isFeatured ? "size-9" : "size-6",
												)}
											/>
										</div>
									</div>
								{/if}

								<!--
								  Module tag (Capture, Edit, Export, …). Pinned to the
								  bottom-right of the visual. Mirrors the "Notion / Google /
								  OpenAI" brand badges on the reference vendor layout.
								-->
								<span
									class="absolute bottom-2.5 right-2.5 inline-flex items-center gap-1 rounded-full bg-foreground/85 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.14em] text-background shadow-craft-sm"
								>
									{item.tag}
								</span>
							</div>

							<!--
							  Text panel. Title + description + "Learn more" link. Padded
							  inside the visual border so the card stays one piece.
							-->
							<div
								class={cn(
									"flex min-w-0 flex-1 flex-col",
									isFeatured ? "gap-3 p-5 sm:p-6 lg:gap-4 lg:p-8" : "mt-5 gap-2",
								)}
							>
								<h3
									class={cn(
										"font-semibold tracking-tight text-foreground",
										isFeatured
											? "text-lg sm:text-xl lg:text-2xl"
											: "text-sm",
									)}
								>
									{item.title}
								</h3>
								<p
									class={cn(
										"text-pretty leading-relaxed text-muted-foreground",
										isFeatured
											? "text-sm sm:text-base"
											: "text-xs",
									)}
								>
									{item.description}
								</p>
								<a
									href={item.href}
									class={cn(
										"mt-auto inline-flex items-center gap-1 self-start text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/70 transition-colors hover:text-foreground",
										isFeatured ? "pt-2 text-xs" : "pt-1",
									)}
								>
									Learn more
									<ArrowRight class="size-3 transition-transform group-hover:translate-x-0.5" />
								</a>
							</div>
						</article>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Final CTA: platform-split downloads, same pattern as the landing page. -->
	<Section id="cta" class="border-t border-border-low/60">
		<Container>
			<Reveal>
				<div
					class="glass-card relative overflow-hidden rounded-[2rem] px-6 py-16 sm:px-14 sm:py-20 md:py-24"
					style="box-shadow: inset 0 1px 0 0 color-mix(in srgb, white 12%, transparent), inset 0 -1px 0 0 color-mix(in srgb, var(--color-foreground) 4%, transparent);"
				>
			

					<div class="relative mx-auto flex max-w-3xl flex-col items-center text-center">
						<div class="glass-chip inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/80">
							<span class="relative flex size-1.5">
								<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-70"></span>
								<span class="relative inline-flex size-1.5 rounded-full bg-primary"></span>
							</span>
							v0.2 ready when you are
						</div>

						<h2 class="text-balance mt-8 text-4xl font-semibold leading-[1.02] tracking-tight text-foreground sm:text-5xl md:text-6xl lg:text-[4.25rem]">
							Skip the editor.
							<span class="block font-medium italic text-foreground/40">Ship the demo.</span>
						</h2>

						<p class="text-pretty mt-7 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Free forever. No account. Windows is daily-driver stable, macOS and Linux are in active beta.
						</p>

						<div class="mt-10 flex w-full flex-col items-stretch gap-3 sm:w-auto sm:flex-row sm:flex-wrap sm:items-center sm:justify-center sm:gap-3">
							{#each platforms as p}
								{@const Icon = p.icon}
								{@const chip = stabilityChip[p.stability]}
								{@const isPrimary = p.stability === "stable"}
								<Button
									href={`/download?os=${p.label.toLowerCase()}`}
									size="lg"
									variant={isPrimary ? "default" : "outline"}
									class="gap-2.5"
								>
									<Icon class="size-4" />
									Download for {p.label}
									<span class={cn("ml-1 inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-[0.14em] ring-1 ring-inset", chip.cls)}>
										{chip.label}
									</span>
								</Button>
							{/each}
						</div>

						<a
							href="/changelog"
							class="group/cta mt-5 inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground transition-colors hover:text-foreground"
						>
							See what's in v0.2
							<ArrowRight class="size-3.5 transition-transform group-hover/cta:translate-x-0.5" />
						</a>
					</div>
				</div>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
