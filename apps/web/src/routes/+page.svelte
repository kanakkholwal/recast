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
	import { toast } from "@recast/ui/sonner";
	import { cn } from "@recast/ui/utils";
	import {
	  ArrowRight,
	  BarChart3,
	  Camera,
	  Check,
	  Cloud,
	  Download,
	  Highlighter,
	  Layout,
	  Link2,
	  Lock,
	  MonitorPlay,
	  MousePointer2,
	  Play,
	  Rocket,
	  Scissors,
	  Search,
	  Sparkles,
	  Target,
	  VolumeX,
	  X,
	  Zap,
	} from "lucide-svelte";
	import { cubicOut } from "svelte/easing";
	import { fly, slide } from "svelte/transition";

	// Step 3 — Share waitlist (Recast Cloud not shipped yet)
	let email = $state("");
	let joined = $state(false);
	let loading = $state(false);
	async function joinWaitlist(e: SubmitEvent) {
		e.preventDefault();
		if (!email.trim()) return;
		loading = true;
		try {
			const res = await fetch("/api/waitlist", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ email, source: "home-cloud" }),
			});
			const data = (await res.json().catch(() => ({}))) as {
				ok?: boolean;
				error?: string;
			};
			if (!data.ok) {
				toast.error(data.error ?? "Couldn't join the waitlist.");
				return;
			}
			joined = true;
		} catch {
			toast.error("Network error. Try again in a moment.");
		} finally {
			loading = false;
		}
	}

	const founderUse = [
		{
			icon: Rocket,
			title: "For solo founders",
			description:
				"Investor walkthroughs and product demos that look funded. Record one between two meetings, ship it before the third.",
		},
		{
			icon: Sparkles,
			title: "For indie hackers",
			description:
				"Launch videos, changelog clips, and Twitter cuts on your own schedule. Fully offline. Ship at midnight, fix typos at 2 AM, nobody knows.",
		},
		{
			icon: MonitorPlay,
			title: "For solopreneurs",
			description:
				"Onboarding videos and support replies that answer once and convert forever. Make it once. Let it work while you sleep.",
		},
	];

	// "OS recorder stops at a file" — contrast rows
	const contrast = [
		{ os: "A raw .mp4 dumped on your desktop", recast: "A polished demo, framed and padded" },
		{ os: "A jittery, distracting cursor", recast: "Cursor smoothed and snapped to targets" },
		{ os: "You, manually trimming in iMovie", recast: "Trim, zoom, and backgrounds, all built in" },
		{ os: "Drag the file into Drive and hope", recast: "One link, with watch analytics" },
	];

	const polishFeatures = [
		{ icon: MousePointer2, title: "Cursor refinement", description: "Velocity smoothing kills twitchy paths and snaps to interactive targets." },
		{ icon: Layout, title: "Auto layouts", description: "Padding, backgrounds, and framing applied live as you record." },
		{ icon: Zap, title: "Smart zoom", description: "Recast zooms toward the action so viewers never miss the point." },
		{ icon: Scissors, title: "Trim & ship", description: "Cut dead frames and export hardware-encoded MP4 in seconds." },
	];

	const shareFeatures = [
		{ icon: Link2, title: "One shareable link", description: "Send a demo without uploads, exports, or attachments." },
		{ icon: BarChart3, title: "Watch analytics", description: "See who watched, how far they got, and what they replayed." },
		{ icon: Lock, title: "Password & expiry", description: "Lock investor demos down, or set them to expire on their own." },
	];

	// "Inside the editor" — honest tour of every tool a non-editor user will
	// actually touch. Each card is tagged `auto` (it happens for you) or
	// `manual` (you reach for it when you want control). The placeholder slot
	// renders a dashed empty frame today so the layout is finalised before the
	// real screenshots land — flip `image` to a path under /static when ready.
	type FeatureKind = "auto" | "manual";
	const editorFeatures: Array<{
		kind: FeatureKind;
		icon: typeof Target;
		title: string;
		description: string;
		image: string | null;
	}> = [
		{
			kind: "auto",
			icon: Target,
			title: "Smart zoom on clicks",
			description:
				"Recast watches your cursor, reads clicks and dwell, and zooms toward the moment that matters. You set zero keyframes.",
			image: "/screenshots/preview_zoom.png",
		},
		{
			kind: "auto",
			icon: VolumeX,
			title: "Silence trimming",
			description:
				"Detects dead-air segments (quiet audio + still cursor) and offers them up as one-click cuts. Toggle them off any time.",
			image: null,
		},
		{
			kind: "auto",
			icon: MousePointer2,
			title: "Cursor smoothing",
			description:
				"Velocity-aware easing kills the jitter, with optional snap-to-target so the path lands where you meant to point.",
			image: "/screenshots/preview_cursor.png",
		},
		{
			kind: "manual",
			icon: Zap,
			title: "Zoom regions on the timeline",
			description:
				"Drag any moment to add a focus region. The auto picks are just a starting point. Every position, scale, and easing is yours to tweak.",
			image: null,
		},
		{
			kind: "manual",
			icon: Highlighter,
			title: "Annotations & blur",
			description:
				"Drop arrows, rectangles, text, or a privacy blur straight on the frame. Layers live on the timeline alongside everything else.",
			image: null,
		},
		{
			kind: "manual",
			icon: Camera,
			title: "Camera bubble",
			description:
				"Record yourself in a draggable bubble with shape, border, and follow-the-cursor motion. No second app. No green screen.",
			image: null,
		},
	];

	const kindChip: Record<FeatureKind, { label: string; dot: string; ring: string }> = {
		auto: {
			label: "Automatic",
			dot: "bg-emerald-500",
			ring: "text-emerald-600 ring-emerald-500/25 dark:text-emerald-400",
		},
		manual: {
			label: "Manual",
			dot: "bg-primary",
			ring: "text-primary ring-primary/25",
		},
	};
</script>

<svelte:head>
	<title>Recast</title>
	<meta
		name="description"
		content="Recast turns a raw screen capture into a polished, shareable demo. Smart auto-edits + a friendly timeline anyone can drive. macOS, Windows, Linux."
	/>
</svelte:head>

<main class="text-foreground">
	<Hero />

	<!-- Trust strip -->
	<Section spacing="tight" class="border-t border-border-low/60">
		<Container>
			<Reveal variant="blur">
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

	<!-- Contrast: your OS recorder stops at a file -->
	<Section id="why" class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Why not the built-in recorder"
				title="Your OS recorder stops at a file."
				description="Every laptop ships a screen recorder. None of them ship a demo. The space between a raw capture and something worth sending is the entire job Recast does for you."
				align="center"
			/>

			<div class="mx-auto mt-14 max-w-3xl overflow-hidden rounded-2xl border border-border-low/50">
				<div class="grid grid-cols-2 border-b border-border-low/50 bg-foreground/2 text-[11px] font-semibold uppercase tracking-[0.16em]">
					<div class="flex items-center gap-2 px-5 py-3 text-muted-foreground">
						<X class="size-3.5" /> Built-in recorder
					</div>
					<div class="flex items-center gap-2 border-l border-border-low/50 px-5 py-3 text-primary">
						<Sparkles class="size-3.5" /> Recast
					</div>
				</div>
				{#each contrast as row, i}
					<Reveal variant={i % 2 === 0 ? "left" : "right"} delay={i * 70}>
						<div class="grid grid-cols-2 {i < contrast.length - 1 ? 'border-b border-border-low/40' : ''}">
							<div class="px-5 py-4 text-sm text-muted-foreground">{row.os}</div>
							<div class="flex items-start gap-2.5 border-l border-border-low/40 bg-primary/4 px-5 py-4 text-sm text-foreground">
								<Check class="mt-0.5 size-4 shrink-0 text-primary" />
								{row.recast}
							</div>
						</div>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Step 1 — Record -->
	<Section id="record" class="border-t border-border-low/60">
		<Container>
			<div class="grid items-center gap-14 lg:grid-cols-12 lg:gap-20">
				<div class="lg:col-span-5">
					<SectionHeader
						eyebrow="Step 1 · Record"
						title="Hit record. That's the whole setup."
						description="Pick a region, a window, or your full screen, then start capturing with one shortcut. No projects to configure. No codecs to pick. No account to create."
					/>
					<div class="mt-10 flex items-center gap-3">
						<Button href="/download" class="gap-2">
							<Download class="size-4" />
							Download free
						</Button>
					</div>
				</div>

				<div class="lg:col-span-7">
					<Reveal variant="morph">
						<article class="glass-card flex flex-col gap-6 rounded-2xl p-7">
							<div class="relative rounded-xl border border-border-low/60 bg-background/60 p-4 shadow-craft-inset">
								<div class="flex items-center gap-3 rounded-lg border border-border-low/60 bg-background/80 px-3 py-2.5">
									<Search class="size-4 text-muted-foreground" />
									<span class="text-sm font-medium text-foreground/85">Start a recording…</span>
									<span class="ml-auto rounded-md border border-border-low/60 bg-background px-1.5 py-0.5 font-mono text-[10px] font-semibold text-muted-foreground">⌘ ⇧ R</span>
								</div>
								<div class="mt-3 space-y-1.5">
									{#each [{ icon: MonitorPlay, label: "Record full screen" }, { icon: Layout, label: "Record region" }, { icon: Play, label: "Continue last project" }] as opt, i}
										{@const Icon = opt.icon}
										<div class="flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors {i === 0 ? 'bg-primary/10 text-foreground' : 'text-muted-foreground'}">
											<Icon class="size-3.5" />
											<span class="font-medium">{opt.label}</span>
										</div>
									{/each}
								</div>
							</div>
						</article>
					</Reveal>
				</div>
			</div>
		</Container>
	</Section>

	<!-- Step 2 — Auto-polish -->
	<Section id="polish" class="border-t border-border-low/60 bg-foreground/1.5 dark:bg-foreground/2">
		<Container>
			<SectionHeader
				eyebrow="Step 2 · Auto-polish"
				title="The editing happens while you record."
				description="Cursor smoothing, padding, backgrounds, zoom, and silence cuts happen while you record. By the time you stop, the demo is mostly done. Open the timeline only when you want to nudge something, and even then, it's the lightest editor you've ever opened."
				align="center"
			/>

			<div class="mt-16 grid grid-cols-1 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-2 lg:grid-cols-4">
				{#each polishFeatures as feature, i}
					{@const Icon = feature.icon}
					<Reveal variant="morph" delay={i * 80} class="h-full">
						<div class="flex h-full flex-col gap-3 bg-background/50 p-6 backdrop-blur-md">
							<Icon class="size-5 text-primary" />
							<div>
								<div class="text-sm font-semibold text-foreground">{feature.title}</div>
								<div class="mt-1.5 text-xs leading-relaxed text-muted-foreground">{feature.description}</div>
							</div>
						</div>
					</Reveal>
				{/each}
			</div>

			<Reveal variant="scale" class="mt-12">
				<div
					class="glass-card relative mx-auto max-w-5xl overflow-hidden rounded-2xl shadow-craft-xl"
					style="transform: perspective(1600px) rotateX(2deg);"
				>
					<div class="flex h-10 items-center gap-2 border-b border-border-low/40 bg-white/5 px-4">
						<div class="flex gap-1.5">
							<span class="size-2.5 rounded-full bg-foreground/15"></span>
							<span class="size-2.5 rounded-full bg-foreground/15"></span>
							<span class="size-2.5 rounded-full bg-foreground/15"></span>
						</div>
						<span class="ml-3 text-[11px] font-medium text-muted-foreground">Recast · Editor</span>
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
		</Container>
	</Section>

	<!-- Inside-the-editor tour. Horizontal scroll rail (not a grid) so each
	     feature gets full-width attention; the screenshots/icons are tilted
	     in 3D space so the section reads as a tools showcase, not a spec
	     sheet. Cards extend past the Container's max-width on both edges,
	     fading into the background to suggest "scroll for more". -->
	<Section id="editor" class="overflow-hidden border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="What's in the editor"
				title="Every tool you need. None of the learning curve."
				description="Smart defaults cover most of what a demo needs. When you want to nudge something, the timeline is small, friendly, and deliberately not a real editor. Drag, drop, done."
				align="center"
			/>
		</Container>

		<div class="relative mt-14">
			<!-- Edge fades. Anchored to the viewport so the rail dissolves into
			     the page background instead of ending in a hard cut. -->
			<div
				class="pointer-events-none absolute inset-y-0 left-0 z-20 w-16 bg-linear-to-r from-background to-transparent sm:w-28"
			></div>
			<div
				class="pointer-events-none absolute inset-y-0 right-0 z-20 w-16 bg-linear-to-l from-background to-transparent sm:w-28"
			></div>

			<!-- The rail. `--rail-inset` keeps the first card aligned with the
			     Container gutter on wide viewports while letting later cards
			     flow off-screen. `scrollbar-hide` keeps the chrome clean — the
			     edge fades + drag cursor already telegraph scrollability. -->
			<div
				class="editor-rail flex snap-x snap-mandatory gap-5 overflow-x-auto py-10 sm:gap-7"
				style="--rail-inset: max(1.25rem, calc((100vw - 80rem) / 2 + 1.25rem)); padding-inline: var(--rail-inset);"
			>
				{#each editorFeatures as feature, i}
					{@const Icon = feature.icon}
					{@const chip = kindChip[feature.kind]}
					<Reveal variant="morph" delay={i * 60} class="snap-center shrink-0">
						<article
							class="group/feat relative flex w-[280px] flex-col gap-5 sm:w-[320px]"
						>
							<!-- Tilted visual. 3D perspective on the wrapper, the inner
							     plate carries the rotation so hover can soften it. -->
							<div
								class="relative h-52 overflow-hidden rounded-2xl border border-border-low/50 bg-linear-to-br from-foreground/[0.05] via-foreground/[0.02] to-transparent shadow-craft-lg transition-shadow duration-500 group-hover/feat:shadow-craft-xl"
								style="perspective: 1200px;"
							>
								<!-- Dot grid backdrop. Faint, decorative — the techy vibe. -->
								<div
									aria-hidden="true"
									class="pointer-events-none absolute inset-0 opacity-50"
									style="background-image: radial-gradient(circle, color-mix(in srgb, var(--color-foreground) 8%, transparent) 1px, transparent 1px); background-size: 16px 16px;"
								></div>

								<!-- Per-card primary glow blob. Sits behind the icon/image. -->
								<div
									aria-hidden="true"
									class="pointer-events-none absolute -bottom-12 left-1/2 size-48 -translate-x-1/2 rounded-full opacity-70"
									style="background: radial-gradient(closest-side, color-mix(in srgb, var(--color-primary) 22%, transparent), transparent 75%);"
								></div>

								<!-- Corner accents. Tiny CRT-ish brackets to frame the
								     plate without surrounding it in a full border. -->
								<span
									aria-hidden="true"
									class="pointer-events-none absolute left-3 top-3 size-3 border-l border-t border-foreground/30"
								></span>
								<span
									aria-hidden="true"
									class="pointer-events-none absolute right-3 top-3 size-3 border-r border-t border-foreground/30"
								></span>
								<span
									aria-hidden="true"
									class="pointer-events-none absolute bottom-3 left-3 size-3 border-b border-l border-foreground/30"
								></span>
								<span
									aria-hidden="true"
									class="pointer-events-none absolute bottom-3 right-3 size-3 border-b border-r border-foreground/30"
								></span>

								{#if feature.image}
									<!-- Real screenshot in a tilted plate. Hover eases the
									     tilt down so the user can see the image flatter. -->
									<div
										class="absolute inset-6 origin-center overflow-hidden rounded-lg border border-border-low/60 shadow-craft-md transition-transform duration-500 group-hover/feat:scale-[1.02]"
										style="transform: perspective(900px) rotateX(6deg) rotateY(-10deg); transform-origin: 50% 70%;"
									>
										<img
											src={feature.image}
											alt={feature.title}
											loading="lazy"
											decoding="async"
											class="block size-full object-cover"
										/>
									</div>
								{:else}
									<!-- Icon-as-hero placeholder. The feature's own glyph
									     sits centred and tilted, so a card without a
									     screenshot still carries identity instead of a "no
									     image" hole. -->
									<div
										class="absolute inset-0 grid place-items-center"
										style="transform: perspective(900px) rotateX(8deg) rotateY(-10deg); transform-origin: 50% 70%;"
									>
										<div
											class="relative grid size-28 place-items-center rounded-2xl border border-border-low/60 bg-card/40 shadow-craft-md backdrop-blur-sm"
										>
											<Icon
												class="size-12 text-foreground/85 drop-shadow-[0_4px_12px_color-mix(in_srgb,var(--color-primary)_35%,transparent)]"
											/>
										</div>
									</div>
								{/if}

								<!-- Mono tag pinned bottom-left, like a chip label on a
								     dev tool. Carries the feature kind for skimmability. -->
								<span
									class={cn(
										"absolute bottom-3 left-1/2 -translate-x-1/2 inline-flex items-center gap-1.5 rounded-full bg-background/70 px-2 py-0.5 font-mono text-[9.5px] font-bold uppercase tracking-[0.14em] ring-1 ring-inset backdrop-blur",
										chip.ring,
									)}
								>
									<span class={cn("size-1.5 rounded-full", chip.dot)}></span>
									{chip.label}
								</span>
							</div>

							<!-- Card content sits below the visual, no enclosing card.
							     Lets the rail feel airier than a tile grid would. -->
							<div class="flex flex-col gap-2 px-1">
								<div class="flex items-center gap-2">
									<span
										class="glass-chip grid size-7 place-items-center rounded-md text-foreground/80 transition-colors group-hover/feat:text-primary"
									>
										<Icon class="size-3.5" />
									</span>
									<h3 class="text-[15px] font-semibold tracking-tight text-foreground">
										{feature.title}
									</h3>
								</div>
								<p class="text-sm leading-relaxed text-muted-foreground">
									{feature.description}
								</p>
							</div>
						</article>
					</Reveal>
				{/each}
			</div>
		</div>

		<Container>
			<Reveal variant="up" delay={150}>
				<p class="mx-auto mt-6 max-w-3xl text-center text-sm leading-relaxed text-muted-foreground">
					Plus trim &amp; cut, background &amp; padding, drop shadow, watermark, custom export presets, and a focus mode that hides everything but the frame. Nothing locked behind a "Pro" tier.
				</p>
			</Reveal>
		</Container>
	</Section>

	<!-- Step 3 — Share (Recast Cloud — waitlist) -->
	<Section id="share" class="border-t border-border-low/60">
		<Container>
			<div class="grid items-center gap-14 lg:grid-cols-12 lg:gap-20">
				<div class="lg:col-span-6">
					<SectionHeader
						eyebrow="Step 3 · Share"
						title="Send a link, not a 200 MB file."
						description="Recast Cloud turns a finished demo into a hosted link with watch analytics built in. No export-upload-attach dance, no Drive permissions emailed back and forth."
					/>

					<ul class="mt-10 space-y-3.5">
						{#each shareFeatures as f, i}
							{@const Icon = f.icon}
							<Reveal as="li" variant="left" delay={i * 70} class="flex items-start gap-3.5">
								<span class="glass-chip mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg text-primary">
									<Icon class="size-4" />
								</span>
								<span>
									<span class="text-sm font-semibold text-foreground">{f.title}</span>
									<span class="block text-sm leading-relaxed text-muted-foreground">{f.description}</span>
								</span>
							</Reveal>
						{/each}
					</ul>
				</div>

				<div class="lg:col-span-6">
					<Reveal variant="morph">
						<div class="glass-card relative overflow-hidden rounded-2xl p-7 shadow-craft-lg sm:p-9">
							<div
								aria-hidden="true"
								class="pointer-events-none absolute -top-24 right-0 size-72 rounded-full opacity-60"
								style="background: radial-gradient(closest-side, color-mix(in srgb, var(--color-primary) 14%, transparent), transparent 70%);"
							></div>

							<div class="relative">
								<span class="glass-chip inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-foreground/80">
									<Cloud class="size-3.5 text-primary" />
									Recast Cloud · coming soon
								</span>

								<h3 class="mt-6 text-2xl font-semibold tracking-tight text-foreground">
									Get early access.
								</h3>
								<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
									The app is free and shipping today. Cloud sharing is on the way.
									Drop your email and we'll save you a spot before the public list opens.
								</p>

								{#if joined}
									<div
										class="mt-7 flex items-center gap-3 rounded-xl border border-primary/30 bg-primary/8 px-4 py-3.5"
										in:fly={{ y: 8, duration: 400, easing: cubicOut }}
									>
										<span class="grid size-7 place-items-center rounded-full bg-primary/15 text-primary">
											<Check class="size-4" />
										</span>
										<span class="text-sm font-medium text-foreground">
											You're on the list. We'll be in touch.
										</span>
									</div>
								{:else}
									<form
										class="mt-7 flex flex-col gap-2.5 sm:flex-row"
										onsubmit={joinWaitlist}
										out:slide={{ duration: 250 }}
									>
										<input
											type="email"
											required
											bind:value={email}
											placeholder="founder@startup.com"
											class="flex-1 rounded-lg border border-border-low/70 bg-background/80 px-3.5 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
										/>
										<Button type="submit" disabled={loading} class="gap-2">
											{loading ? "Joining…" : "Join waitlist"}
											<ArrowRight class="size-4" />
										</Button>
									</form>
								{/if}

								<p class="mt-4 text-xs text-muted-foreground">
									Free tier includes <span class="font-semibold text-foreground">10 shareable links</span>. No card, ever.
								</p>
							</div>
						</div>
					</Reveal>
				</div>
			</div>
		</Container>
	</Section>

	<!-- Built for solo founders -->
	<Section id="founders" class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Built for founders"
				title="Shaped for the people shipping solo."
				description="Opinionated where it matters, out of your way everywhere else. Auto-polish for the 80 % case, a minimal timeline for the moments you actually want to control."
				align="center"
			/>

			<div class="mt-16 grid grid-cols-1 gap-4 md:grid-cols-3">
				{#each founderUse as item, i}
					{@const Icon = item.icon}
					<Reveal variant="morph" delay={i * 90}>
						<article class="glass-card group flex h-full flex-col rounded-2xl p-7 transition-all duration-300 hover:-translate-y-1 hover:shadow-craft-lg">
							<span class="glass-chip grid size-11 place-items-center rounded-xl text-foreground/70 transition-colors group-hover:text-primary">
								<Icon class="size-5" />
							</span>
							<h3 class="mt-6 text-lg font-semibold tracking-tight text-foreground">
								{item.title}
							</h3>
							<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
								{item.description}
							</p>
						</article>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Pricing teaser -->
	<Section id="pricing-teaser" class="border-t border-border-low/60">
		<Container>
			<div class="grid gap-4 md:grid-cols-2">
				<Reveal variant="left">
					<article class="glass-card flex h-full flex-col rounded-2xl p-8">
						<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
							The app
						</span>
						<div class="mt-2 flex items-baseline gap-2">
							<span class="text-4xl font-semibold tracking-tight text-foreground">Free</span>
							<span class="text-sm text-muted-foreground">forever</span>
						</div>
						<p class="mt-3 text-sm leading-relaxed text-muted-foreground">
							Record, auto-polish, edit, and export, all of it offline and without an account. The whole recorder, no asterisk.
						</p>
						<div class="mt-7">
							<Button href="/download" variant="outline" class="gap-2">
								<Download class="size-4" />
								Download
							</Button>
						</div>
					</article>
				</Reveal>

				<Reveal variant="right" delay={80}>
					<article class="glass-card relative flex h-full flex-col overflow-hidden rounded-2xl p-8 ring-1 ring-primary/20">
						<div
							aria-hidden="true"
							class="pointer-events-none absolute -right-10 -top-10 size-48 rounded-full bg-primary/10 blur-2xl"
						></div>
						<span class="relative text-[11px] font-semibold uppercase tracking-[0.16em] text-primary">
							Recast Cloud
						</span>
						<div class="relative mt-2 flex items-baseline gap-2">
							<span class="text-4xl font-semibold tracking-tight text-foreground">Sharing</span>
							<span class="text-sm text-muted-foreground">+ analytics</span>
						</div>
						<p class="relative mt-3 text-sm leading-relaxed text-muted-foreground">
							Hosted links, watch analytics, custom branding, and password-protected demos. Coming soon. Pricing follows.
						</p>
						<div class="relative mt-7">
							<Button href="/pricing" class="group/cta gap-2">
								See what's planned
								<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
							</Button>
						</div>
					</article>
				</Reveal>
			</div>
		</Container>
	</Section>

	<!-- Final CTA -->
	<Section id="cta" class="border-t border-border-low/60">
		<Container>
			<Reveal variant="scale">
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
							A demo, not a project.
							<span class="block font-medium italic text-foreground/40">Ship it the same day.</span>
						</h2>

						<p class="text-pretty mt-7 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Free forever. No account. Three platforms. A timeline you'll actually want to open.
						</p>

						<div class="mt-10 flex flex-col items-center gap-3 sm:flex-row sm:gap-4">
							<Button href="/download" size="lg" class="gap-2.5">
								<Download class="size-4" />
								Download free
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

<style>
	/* Editor-tour rail: hide the scrollbar (edge fades + drag cursor already
	   telegraph scrollability) and lean on grab/grabbing cursors so the rail
	   reads as draggable on first encounter. */
	.editor-rail {
		scrollbar-width: none;
		-ms-overflow-style: none;
		cursor: grab;
		scroll-behavior: smooth;
	}
	.editor-rail::-webkit-scrollbar {
		display: none;
	}
	.editor-rail:active {
		cursor: grabbing;
	}
</style>
