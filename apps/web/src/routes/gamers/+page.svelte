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
		Download,
		Film,
		Gamepad2,
		MessageCircle,
		MousePointer2,
		Scissors,
		Sparkles,
		Trophy,
		Wand2,
		Zap,
	} from "lucide-svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	const words = ["highlights.", "montages.", "clips.", "guides.", "best moments."];

	const flow = [
		{ icon: Gamepad2, title: "Capture the run", description: "Native capture at full framerate — record the whole session or just the last play." },
		{ icon: Wand2, title: "Auto-polish the clip", description: "Cursor and camera smoothing, auto zoom, and clean framing applied as you record." },
		{ icon: Scissors, title: "Cut to the moment", description: "Trim down to the highlight, speed-ramp the boring parts, export in seconds." },
		{ icon: MessageCircle, title: "Drop it in chat", description: "Share a finished clip straight to Discord — no render queue, no re-encode." },
	];

	const useCases = [
		{ icon: Trophy, title: "Highlight reels", description: "Stitch your best plays into a montage worth posting." },
		{ icon: Film, title: "How-to guides", description: "Walkthroughs and strats your guildmates will actually watch." },
		{ icon: Zap, title: "Clip & react", description: "Grab the moment the second it happens — bound to a hotkey." },
	];
</script>

<svelte:head>
	<title>Recast for Gamers — Clips and montages, auto-polished</title>
	<meta
		name="description"
		content="Recast captures your gameplay and auto-polishes it into clips, highlights, and montages — then ships them to Discord. Free, offline, on macOS, Windows, and Linux."
	/>
</svelte:head>

<main class="text-foreground">
	<!-- Hero -->
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-20 md:pt-44 md:pb-24">
		<div
			aria-hidden="true"
			class="pointer-events-none absolute inset-0 -z-10"
			style="background: radial-gradient(ellipse 80% 50% at 50% -10%, color-mix(in srgb, var(--color-primary) 10%, transparent), transparent 72%);"
		></div>
		<Container>
			<div class="mx-auto flex max-w-4xl flex-col items-center text-center">
				<Eyebrow icon={Gamepad2} variant="primary">For gamers</Eyebrow>
				<h1
					class="text-balance mt-7 text-5xl font-semibold leading-[1.04] tracking-tight text-foreground sm:text-6xl md:text-7xl"
					in:fly={{ y: 16, duration: 720, delay: 80, easing: cubicOut }}
				>
					Capture the play.
					<span class="mt-2 flex justify-center font-medium italic text-foreground/40">
						<span class="inline-grid overflow-hidden">
							<TextLoop class="text-primary" texts={words} interval={2800} />
						</span>
					</span>
				</h1>
				<p
					class="text-pretty mt-7 max-w-2xl text-base leading-relaxed text-muted-foreground sm:text-lg"
					in:fly={{ y: 16, duration: 720, delay: 200, easing: cubicOut }}
				>
					Recast records your gameplay and auto-polishes it into clips, highlights, and
					montages — then hands you a finished video to drop in chat. No editor required.
				</p>
				<div
					class="mt-9 flex flex-col items-center gap-3 sm:flex-row sm:gap-4"
					in:fly={{ y: 16, duration: 720, delay: 320, easing: cubicOut }}
				>
					<Button href="/download" size="lg" class="gap-2.5">
						<Download class="size-4" />
						Download free
					</Button>
					<Button href="/" variant="outline" size="lg" class="group/cta gap-2">
						Recast for founders
						<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
					</Button>
				</div>
			</div>
		</Container>
	</Section>

	<!-- Flow -->
	<Section class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Run → Clip → Share"
				title="From last play to posted clip."
				description="Same auto-polish engine the founders use — tuned for gameplay instead of product demos."
				align="center"
			/>
			<div class="mt-16 grid grid-cols-1 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-2 lg:grid-cols-4">
				{#each flow as step, i}
					{@const Icon = step.icon}
					<Reveal variant="morph" delay={i * 80} class="h-full">
						<div class="flex h-full flex-col gap-3 bg-background/50 p-6 backdrop-blur-md">
							<div class="flex items-center gap-2">
								<Icon class="size-5 text-primary" />
								<span class="font-mono text-[11px] font-semibold text-muted-foreground">0{i + 1}</span>
							</div>
							<div>
								<div class="text-sm font-semibold text-foreground">{step.title}</div>
								<div class="mt-1.5 text-xs leading-relaxed text-muted-foreground">{step.description}</div>
							</div>
						</div>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Use cases -->
	<Section class="border-t border-border-low/60 bg-foreground/1.5 dark:bg-foreground/2">
		<Container>
			<SectionHeader
				eyebrow="What gamers ship with Recast"
				title="Clips that look like the channel, not the capture card."
				align="center"
			/>
			<div class="mt-16 grid grid-cols-1 gap-4 md:grid-cols-3">
				{#each useCases as item, i}
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

	<!-- Why -->
	<Section class="border-t border-border-low/60">
		<Container>
			<div class="grid items-center gap-14 lg:grid-cols-2 lg:gap-20">
				<div>
					<SectionHeader
						eyebrow="Why Recast over OBS"
						title="Less rig. More highlight."
						description="OBS is a broadcast studio you have to configure. Recast is one shortcut and a finished clip — built for the moment you just want to capture, cut, and post."
					/>
				</div>
				<div class="grid gap-3">
					{#each [
						{ icon: Cpu, title: "No scene setup", description: "Hit record. No sources, no canvas, no profiles to build first." },
						{ icon: MousePointer2, title: "Auto-polished", description: "Smoothing, zoom, and framing happen for you — no editor pass." },
						{ icon: Sparkles, title: "Finished, not raw", description: "You end with a clip ready to post, not a file to process." },
					] as item, i}
						{@const Icon = item.icon}
						<Reveal variant="left" delay={i * 70}>
							<div class="glass-card flex items-start gap-4 rounded-xl p-5">
								<span class="glass-chip grid size-10 shrink-0 place-items-center rounded-lg text-primary">
									<Icon class="size-4" />
								</span>
								<div>
									<div class="text-sm font-semibold text-foreground">{item.title}</div>
									<div class="mt-1 text-xs leading-relaxed text-muted-foreground">{item.description}</div>
								</div>
							</div>
						</Reveal>
					{/each}
				</div>
			</div>
		</Container>
	</Section>

	<!-- CTA -->
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
					<div class="relative mx-auto flex max-w-3xl flex-col items-center text-center">
						<h2 class="text-balance text-4xl font-semibold leading-[1.04] tracking-tight text-foreground sm:text-5xl md:text-6xl">
							Stop recording.
							<span class="block font-medium italic text-foreground/40">Start clipping.</span>
						</h2>
						<p class="text-pretty mt-7 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Free forever. No account. macOS, Windows, and Linux.
						</p>
						<div class="mt-10">
							<Button href="/download" size="lg" class="gap-2.5">
								<Download class="size-4" />
								Download Recast
							</Button>
						</div>
					</div>
				</div>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
