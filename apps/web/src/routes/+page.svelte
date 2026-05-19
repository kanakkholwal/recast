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
		BarChart3,
		Check,
		Cloud,
		Download,
		Gamepad2,
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
		X,
		Zap,
	} from "lucide-svelte";
	import { fly, slide } from "svelte/transition";
	import { cubicOut } from "svelte/easing";

	// Step 3 — Share waitlist (Recast Cloud not shipped yet)
	let email = $state("");
	let joined = $state(false);
	function joinWaitlist(e: SubmitEvent) {
		e.preventDefault();
		if (email.trim()) joined = true;
	}

	const founderUse = [
		{
			icon: Rocket,
			title: "For solo founders",
			description:
				"Investor walkthroughs and product demos that look funded — recorded between two meetings, no editor in sight.",
		},
		{
			icon: Sparkles,
			title: "For indie hackers",
			description:
				"Launch videos, changelog clips, and Twitter cuts shipped on your own schedule, fully offline.",
		},
		{
			icon: MonitorPlay,
			title: "For solopreneurs",
			description:
				"Onboarding videos and support replies that answer once and convert forever.",
		},
	];

	// "OS recorder stops at a file" — contrast rows
	const contrast = [
		{ os: "A raw .mp4 dumped on your desktop", recast: "A polished demo, framed and padded" },
		{ os: "A jittery, distracting cursor", recast: "Cursor smoothed and snapped to targets" },
		{ os: "You, manually trimming in iMovie", recast: "Trim, zoom, and background — built in" },
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
		{ icon: Lock, title: "Password & expiry", description: "Lock investor demos down — or let them expire on their own." },
	];
</script>

<svelte:head>
	<title>Recast — Turn a screen capture into a demo that ships itself</title>
	<meta
		name="description"
		content="Recast turns a raw screen capture into a polished, shareable demo — automatically. The recorder for solo founders who'd rather ship than open a timeline. macOS, Windows, Linux."
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
				description="Every laptop ships a screen recorder. None of them ship a demo. That gap — between a raw capture and something worth sending — is the entire job Recast does for you."
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
						eyebrow="Step 1 — Record"
						title="Hit record. That's the whole setup."
						description="Region, window, or full screen — start capturing with one shortcut. No projects to configure, no codecs to pick, no accounts to create."
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
				eyebrow="Step 2 — Auto-polish"
				title="The editing happens while you record."
				description="Cursor smoothing, padding, backgrounds, and zoom land as the bytes hit disk. By the time you stop recording, your demo is already polished — no timeline gymnastics required."
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
		</Container>
	</Section>

	<!-- Step 3 — Share (Recast Cloud — waitlist) -->
	<Section id="share" class="border-t border-border-low/60">
		<Container>
			<div class="grid items-center gap-14 lg:grid-cols-12 lg:gap-20">
				<div class="lg:col-span-6">
					<SectionHeader
						eyebrow="Step 3 — Share"
						title="Send a link, not a 200 MB file."
						description="Recast Cloud turns a finished demo into a hosted link with watch analytics built in. Skip the export-upload-attach dance entirely."
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
									The app is free and shipping today. Cloud sharing is in the works —
									join the waitlist and we'll hand you a spot first.
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
											You're on the list — we'll be in touch.
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
										<Button type="submit" class="gap-2">
											Join waitlist
											<ArrowRight class="size-4" />
										</Button>
									</form>
								{/if}

								<p class="mt-4 text-xs text-muted-foreground">
									Free tier includes <span class="font-semibold text-foreground">10 shareable links</span> — no card, ever.
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
				description="Recast is opinionated where it matters and out of your way everywhere else — because your time goes to the product, not the timeline."
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

	<!-- Also great for gamers -->
	<Section spacing="tight" class="border-t border-border-low/60">
		<Container>
			<Reveal variant="blur">
				<a
					href="/gamers"
					class="glass-card group flex flex-col items-start gap-6 overflow-hidden rounded-2xl p-7 transition-all duration-300 hover:-translate-y-0.5 hover:shadow-craft-lg sm:flex-row sm:items-center sm:gap-8 sm:p-9"
				>
					<span class="glass-chip grid size-14 shrink-0 place-items-center rounded-2xl text-primary">
						<Gamepad2 class="size-6" />
					</span>
					<div class="flex-1">
						<h3 class="text-xl font-semibold tracking-tight text-foreground">
							Also great for gamers.
						</h3>
						<p class="mt-1.5 text-sm leading-relaxed text-muted-foreground">
							Same engine, built for clips and highlights — capture your run, auto-polish the montage, share it to Discord.
						</p>
					</div>
					<span class="inline-flex items-center gap-2 text-sm font-semibold text-foreground">
						See the gamer setup
						<ArrowRight class="size-4 transition-transform group-hover:translate-x-0.5" />
					</span>
				</a>
			</Reveal>
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
							Record, auto-polish, edit, and export — fully offline, no account. The whole recorder, no asterisk.
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
							Hosted links, watch analytics, custom branding, and password-protected demos. Coming soon — pricing on the way.
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
							Skip the editor.
							<span class="block font-medium italic text-foreground/40">Ship the demo.</span>
						</h2>

						<p class="text-pretty mt-7 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Free forever. No account required. Three platforms. One opinionated tool.
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
