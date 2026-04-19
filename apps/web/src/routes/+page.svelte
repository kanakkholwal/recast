<script lang="ts">
	import {
	  Container,
	  FeatureGrid,
	  Footer,
	  Hero,
	  Navbar,
	  Section,
	} from "$lib/components";
	import { Button } from "@recast/ui/button";
	import { Rocket, Terminal, Users } from "lucide-svelte";
</script>

<svelte:head>
	<title>Recast — Record. Refine. Share.</title>
	<meta
		name="description"
		content="Recast captures your screen with cinematic cursor motion, auto-enhanced visuals, and zero-lag export."
	/>
</svelte:head>

<Navbar />

<main class="bg-background text-foreground/80 selection:bg-primary/10">
	<Hero />

	<Section id="philosophy" class="py-24 md:py-32">
		<Container>
			<div class="max-w-3xl">
				<p
					class="text-[13px] font-semibold text-foreground/30 uppercase tracking-[0.2em] mb-12"
				>
					Design Philosophy
				</p>

				<div class="space-y-24">
					<div
						class="group craft-block hover:bg-secondary dark:hover:bg-white/5 relative"
					>
						<div
							class="absolute -left-4 top-10 size-1 rounded-full bg-foreground/10 invisible-ui"
						></div>
						<h2
							class="text-4xl md:text-5xl font-semibold mb-6 text-foreground"
						>
							Smooth by Default
						</h2>
						<p
							class="text-lg md:text-xl text-foreground/50 leading-relaxed mb-8"
						>
							No jitter. No awkward cursor jumps. We treat motion
							as a first-class citizen, ensuring every recording
							feels intentional and human.
						</p>
						<ul
							class="space-y-4 text-sm font-medium text-foreground/40 ml-1"
						>
							<li class="flex items-center gap-3">
								<div
									class="size-1.5 rounded-full bg-foreground/20"
								></div>
								 velocity-based cursor smoothing
							</li>
							<li class="flex items-center gap-3">
								<div
									class="size-1.5 rounded-full bg-foreground/20"
								></div>
								 natural click feedback
							</li>
							<li class="flex items-center gap-3">
								<div
									class="size-1.5 rounded-full bg-foreground/20"
								></div>
								 motion that feels human
							</li>
						</ul>
					</div>

					<div
						class="group craft-block hover:bg-secondary dark:hover:bg-white/5 relative ml-0 md:ml-12 border-l border-border-low pl-8 md:pl-12"
					>
						<h2
							class="text-4xl md:text-5xl font-semibold mb-6 text-foreground"
						>
							Edit Without Editing
						</h2>
						<p
							class="text-lg md:text-xl text-foreground/50 leading-relaxed mb-8"
						>
							Skip timelines. Stay fast. Recast automates the
							tedious parts of video production so you can focus
							on the story.
						</p>
						<ul
							class="space-y-4 text-sm font-medium text-foreground/40"
						>
							<li class="flex items-center gap-3">
								<div
									class="size-1.5 rounded-full bg-foreground/20"
								></div>
								 trim instantly with smart markers
							</li>
							<li class="flex items-center gap-3">
								<div
									class="size-1.5 rounded-full bg-foreground/20"
								></div>
								 auto framing + responsive padding
							</li>
							<li class="flex items-center gap-3">
								<div
									class="size-1.5 rounded-full bg-foreground/20"
								></div>
								 clean backgrounds in one click
							</li>
						</ul>
					</div>

					<div
						class="group craft-block hover:bg-secondary dark:hover:bg-white/5 relative ml-0 md:ml-24 border-l border-border-low pl-8 md:pl-12"
					>
						<h2
							class="text-4xl md:text-5xl font-semibold mb-6 text-foreground"
						>
							Export in Seconds
						</h2>
						<p
							class="text-lg md:text-xl text-foreground/50 leading-relaxed mb-8"
						>
							No waiting. No rendering pain. Pure
							hardware-accelerated performance that keeps you in
							the flow.
						</p>
						<div
							class="p-6 craft-card w-fit group-hover:scale-[1.02] transition-transform"
						>
							<div
								class="flex items-center gap-4 text-sm font-semibold"
							>
								<Terminal class="size-4 opacity-40" />
								<span>4K Hardware Export Alpha</span>
							</div>
						</div>
					</div>
				</div>
			</div>
		</Container>
	</Section>

	<Section
		id="workflow"
		class="py-32 bg-muted/30 dark:bg-white/[0.02] border-y border-border-low"
	>
		<Container>
			<div class="grid grid-cols-1 lg:grid-cols-2 gap-24 items-start">
				<div class="sticky top-40">
					<h2
						class="text-5xl md:text-6xl font-semibold mb-8 text-foreground tracking-tight"
					>
						The workflow. <br />
						<span class="text-foreground/30">Redefined.</span>
					</h2>
					<p class="text-xl text-foreground/50 leading-relaxed">
						Raw capture is just the beginning. Recast refines your
						output instantly, adding clarity and layout polish.
					</p>
				</div>

				<div class="space-y-16">
					{#each [{ step: "01", title: "Record", desc: "Capture screen, window, or region. We handle the native processing without proxying." }, { step: "02", title: "Enhance", desc: "Recast auto-applies velocity-based cursor smoothing, smart padding, and cleans up backgrounds instantly." }, { step: "03", title: "Share", desc: "Export hardware-accelerated in seconds or save as a local .recast project." }] as { step, title, desc }}
						<div class="group relative pl-12">
							<div
								class="absolute left-0 top-1 text-[13px] font-bold text-foreground/20 group-hover:text-foreground/80 transition-colors"
							>
								{step}
							</div>
							<h4
								class="text-2xl font-semibold mb-3 text-foreground"
							>
								{title}
							</h4>
							<p
								class="text-foreground/50 leading-relaxed text-lg"
							>
								{desc}
							</p>
						</div>
					{/each}
				</div>
			</div>
		</Container>
	</Section>

	<FeatureGrid />

	<Section id="use-cases" class="py-32">
		<Container>
			<div class="max-w-2xl mb-24">
				<h2
					class="text-5xl font-semibold mb-8 text-foreground tracking-tight"
				>
					Use cases.
				</h2>
				<p class="text-xl text-foreground/50">
					Why Recast? Not a heavy editor. Not a laggy recorder. Simply
					better output.
				</p>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-3 gap-8">
				{#each [{ icon: Terminal, title: "For Developers", desc: "Explain bugs, APIs, and flows clearly without typing a thousand-word pull request." }, { icon: Rocket, title: "For Founders", desc: "Create product demos in minutes. Show your investors exactly what you've built." }, { icon: Users, title: "For Teams", desc: "Replace meetings with async clarity. Share pristine updates that actually get watched." }] as item}
					{@const Icon = item.icon}
					<div
						class="group craft-block bg-muted/40 dark:bg-white/[0.02] hover:bg-muted/80 dark:hover:bg-white/[0.05] border border-border-low shadow-craft-sm"
					>
						<div
							class="size-10 rounded-xl bg-background dark:bg-neutral-900 border border-border-low flex items-center justify-center mb-8 group-hover:scale-110 transition-transform"
						>
							<Icon class="size-4 text-foreground/60" />
						</div>
						<h4 class="text-xl font-semibold mb-4 text-foreground">
							{item.title}
						</h4>
						<p class="text-foreground/50 leading-relaxed">
							{item.desc}
						</p>
					</div>
				{/each}
			</div>
		</Container>
	</Section>

	<Section id="cta" class="py-32">
		<Container>
			<div
				class="craft-card p-16 md:p-24 text-center relative overflow-hidden group"
			>
				<div
					class="absolute inset-0 bg-grid-pattern opacity-20 group-hover:opacity-40 transition-opacity"
				></div>
				<div class="relative z-10 max-w-2xl mx-auto">
					<h2
						class="text-5xl md:text-7xl font-semibold mb-10 text-foreground tracking-tight"
					>
						Ready to recast?
					</h2>
					<Button
						size="lg"
						href="/download"
						class="h-16 px-12 text-lg font-bold bg-foreground text-background hover:scale-105 transition-all rounded-2xl shadow-craft-xl"
					>
						Download for Desktop
					</Button>
					<p class="mt-8 text-sm font-medium text-foreground/30">
						Free for personal use. No credit card required.
					</p>
				</div>
			</div>
		</Container>
	</Section>
</main>

<Footer />
