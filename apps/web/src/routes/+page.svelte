<script lang="ts">
	import { Navbar, Hero, FeatureGrid, Section, Container } from "$lib/components";
	import { Button } from "@recast/ui/button";
	import { ArrowRight, CheckCircle2 } from "lucide-svelte";
	import { Sparkles, MousePointer2, Frame } from "lucide-svelte";
</script>

<svelte:head>
	<title>Recast — Record. Refine. Share.</title>
	<meta name="description" content="Recast captures your screen with cinematic cursor motion, auto-enhanced visuals, and zero-lag export." />
</svelte:head>

<Navbar />

<main class="bg-background text-foreground selection:bg-primary/10">
	<Hero />

	<Section id="trust" class="py-12 border-y border-border/40 bg-muted/20">
		<Container class="text-center">
			<p class="text-sm font-medium text-muted-foreground uppercase tracking-widest mb-8">
				Built for people who explain things on screen
			</p>
			<div class="flex flex-wrap justify-center gap-8 md:gap-16 text-muted-foreground font-medium">
				<span>Product demos</span>
				<span>Bug reports</span>
				<span>Tutorials</span>
				<span>Async team updates</span>
			</div>
		</Container>
	</Section>

	<Section id="pillars" class="py-24 md:py-32">
		<Container>
			<div class="grid grid-cols-1 md:grid-cols-3 gap-12 lg:gap-16">
				<div class="space-y-4">
					<div class="size-10 rounded-lg bg-primary/5 border border-border/50 flex items-center justify-center font-semibold text-primary mb-6">01</div>
					<h3 class="text-2xl font-semibold tracking-tight">Smooth by Default</h3>
					<p class="text-muted-foreground leading-relaxed">No jitter. No awkward cursor jumps.</p>
					<ul class="space-y-2 mt-4 text-sm text-muted-foreground">
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> velocity-based cursor smoothing</li>
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> natural click feedback</li>
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> motion that feels human</li>
					</ul>
				</div>
				<div class="space-y-4">
					<div class="size-10 rounded-lg bg-primary/5 border border-border/50 flex items-center justify-center font-semibold text-primary mb-6">02</div>
					<h3 class="text-2xl font-semibold tracking-tight">Edit Without Editing</h3>
					<p class="text-muted-foreground leading-relaxed">Skip timelines. Stay fast.</p>
					<ul class="space-y-2 mt-4 text-sm text-muted-foreground">
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> trim instantly</li>
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> auto framing + padding</li>
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> clean backgrounds in one click</li>
					</ul>
				</div>
				<div class="space-y-4">
					<div class="size-10 rounded-lg bg-primary/5 border border-border/50 flex items-center justify-center font-semibold text-primary mb-6">03</div>
					<h3 class="text-2xl font-semibold tracking-tight">Export in Seconds</h3>
					<p class="text-muted-foreground leading-relaxed">No waiting. No rendering pain.</p>
					<ul class="space-y-2 mt-4 text-sm text-muted-foreground">
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> hardware-accelerated export</li>
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> faster-than-realtime processing</li>
						<li class="flex items-center gap-2"><CheckCircle2 class="size-4 text-primary/70" /> ready to share instantly</li>
					</ul>
				</div>
			</div>
		</Container>
	</Section>

	<Section id="how-it-works" class="py-24 md:py-32 overflow-hidden border-y border-border/40 bg-background">
	<Container>
		<div class="grid grid-cols-1 lg:grid-cols-2 gap-16 lg:gap-24 items-center">
			
			<div class="max-w-xl">
				<h2 class="text-3xl md:text-5xl font-semibold tracking-tight mb-6 leading-[1.1]">
					What you record is not what you send. <br />
					<span class="text-muted-foreground">It's better.</span>
				</h2>
				<p class="text-lg text-muted-foreground mb-12 leading-relaxed">
					Raw capture is just the beginning. Recast refines your output instantly, adding cursor enhancement for clarity and layout polish for a professional look.
				</p>

				<div class="space-y-10 relative">
					<div class="absolute left-[11px] top-4 bottom-4 w-px bg-border z-0"></div>

					<div class="relative pl-12 z-10 group">
						<div class="absolute left-0 top-1 size-6 rounded-full bg-background border border-border flex items-center justify-center transition-colors group-hover:border-foreground/50">
							<div class="size-2 rounded-full bg-muted-foreground/30 group-hover:bg-foreground/50 transition-colors"></div>
						</div>
						<h4 class="text-xl font-semibold tracking-tight mb-2">Record</h4>
						<p class="text-muted-foreground text-sm leading-relaxed">
							Capture screen, window, or region. We handle the native processing without proxying.
						</p>
					</div>

					<div class="relative pl-12 z-10">
						<div class="absolute left-0 top-1 size-6 rounded-full bg-primary/20 flex items-center justify-center ring-4 ring-background">
							<div class="size-2.5 rounded-full bg-primary shadow-[0_0_12px_rgba(0,0,0,0.5)] shadow-primary/50"></div>
						</div>
						<h4 class="text-xl font-semibold tracking-tight mb-2 text-foreground">Enhance</h4>
						<p class="text-muted-foreground text-sm leading-relaxed">
							Recast auto-applies velocity-based cursor smoothing, smart padding, and cleans up backgrounds instantly.
						</p>
					</div>

					<div class="relative pl-12 z-10 group">
						<div class="absolute left-0 top-1 size-6 rounded-full bg-background border border-border flex items-center justify-center transition-colors group-hover:border-foreground/50">
							<div class="size-2 rounded-full bg-muted-foreground/30 group-hover:bg-foreground/50 transition-colors"></div>
						</div>
						<h4 class="text-xl font-semibold tracking-tight mb-2">Share</h4>
						<p class="text-muted-foreground text-sm leading-relaxed">
							Export hardware-accelerated in seconds or save as a local <code class="text-xs bg-muted px-1.5 py-0.5 rounded border border-border/50 text-foreground font-mono">.recast</code> project.
						</p>
					</div>
				</div>
			</div>

			<div class="relative aspect-square md:aspect-4/3 rounded-3xl bg-muted/20 border border-border/50 shadow-2xl overflow-hidden flex flex-col group">
				
				<div class="h-12 border-b border-border/50 flex items-center px-4 gap-2 bg-background/50 backdrop-blur-sm z-20">
					<div class="flex gap-1.5">
						<div class="size-3 rounded-full bg-border/80"></div>
						<div class="size-3 rounded-full bg-border/80"></div>
						<div class="size-3 rounded-full bg-border/80"></div>
					</div>
					<div class="mx-auto text-xs font-medium text-muted-foreground flex items-center gap-2">
						<div class="size-2 rounded-full bg-primary/80 animate-pulse"></div>
						Optimizing your video
					</div>
				</div>

				<div class="flex-1 relative p-6 md:p-10 flex items-center justify-center bg-[radial-gradient(ellipse_at_center,var(--tw-gradient-stops))] from-background via-muted/30 to-muted/50 overflow-hidden">
					
					<div class="absolute top-8 left-8 px-3 py-1.5 rounded-full bg-background border border-border shadow-sm text-xs font-medium flex items-center gap-2 z-20 text-foreground translate-y-4 opacity-0 animate-in fade-in slide-in-from-bottom-2 duration-700 fill-mode-forwards delay-300">
						<Frame class="size-3.5 text-primary" /> Auto Framing
					</div>
					<div class="absolute bottom-8 right-8 px-3 py-1.5 rounded-full bg-background border border-border shadow-sm text-xs font-medium flex items-center gap-2 z-20 text-foreground -translate-y-4 opacity-0 animate-in fade-in slide-in-from-top-2 duration-700 fill-mode-forwards delay-500">
						<Sparkles class="size-3.5 text-primary" /> Layout Polish
					</div>

					<div class="relative w-full max-w-sm aspect-video rounded-xl border border-border bg-card shadow-xl overflow-hidden z-10 transition-transform duration-700 group-hover:scale-105">
						
						<div class="absolute inset-0 bg-[linear-gradient(to_right,#80808012_1px,transparent_1px),linear-gradient(to_bottom,#80808012_1px,transparent_1px)] bg-[size:14px_24px]"></div>
						
						<svg class="absolute inset-0 w-full h-full opacity-30" viewBox="0 0 100 100" preserveAspectRatio="none">
							<path d="M 10 80 L 25 50 L 40 65 L 55 35 L 70 45 L 90 20" fill="none" stroke="currentColor" stroke-width="0.5" stroke-dasharray="2 2" class="text-muted-foreground" />
						</svg>

						<svg class="absolute inset-0 w-full h-full drop-shadow-md" viewBox="0 0 100 100" preserveAspectRatio="none">
							<path d="M 10 80 C 25 50, 40 65, 55 35 C 70 45, 80 20, 90 20" fill="none" stroke="currentColor" stroke-width="2" class="text-primary" stroke-linecap="round" />
						</svg>

						<div class="absolute left-[15%] top-[25%] w-[40%] h-[30%] rounded bg-muted border border-border/50 shadow-sm opacity-80"></div>
						
						<div class="absolute top-[20%] left-[90%] -translate-x-1/2 -translate-y-1/2 z-30 drop-shadow-xl text-foreground">
							<MousePointer2 class="size-5 fill-background" />
							<div class="absolute top-0 left-0 size-full bg-primary/40 rounded-full blur-md animate-ping"></div>
						</div>
					</div>

					<div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-64 h-64 bg-primary/10 rounded-full blur-3xl -z-10"></div>
				</div>
			</div>
			
		</div>
	</Container>
</Section>

	<FeatureGrid />

	<Section id="use-cases" class="py-24 md:py-32">
		<Container>
			<div class="mb-16">
				<h2 class="text-3xl font-semibold tracking-tight mb-4">Built for clarity. Designed for speed.</h2>
				<p class="text-muted-foreground">Why Recast? Not a heavy editor. Not a laggy recorder. Not cloud-dependent.</p>
			</div>
			
			<div class="grid grid-cols-1 md:grid-cols-3 gap-8">
				<div class="p-8 rounded-2xl bg-muted/20 border border-border/40">
					<h4 class="text-xl font-semibold mb-2">For Developers</h4>
					<p class="text-muted-foreground">Explain bugs, APIs, and flows clearly.</p>
				</div>
				<div class="p-8 rounded-2xl bg-muted/20 border border-border/40">
					<h4 class="text-xl font-semibold mb-2">For Founders</h4>
					<p class="text-muted-foreground">Create product demos in minutes.</p>
				</div>
				<div class="p-8 rounded-2xl bg-muted/20 border border-border/40">
					<h4 class="text-xl font-semibold mb-2">For Teams</h4>
					<p class="text-muted-foreground">Replace meetings with async clarity.</p>
				</div>
			</div>
		</Container>
	</Section>

	<Section id="cta" class="py-32 bg-primary text-primary-foreground text-center">
		<Container>
			<h2 class="text-4xl md:text-6xl font-semibold tracking-tight mb-8">Stop recording. <br /> Start presenting.</h2>
			<Button size="lg" variant="secondary" class="h-14 px-8 text-lg font-medium shadow-sm group">
				Start Recording
				<ArrowRight class="ml-2 size-5 group-hover:translate-x-1 transition-transform" />
			</Button>
		</Container>
	</Section>
</main>

<footer class="py-12 border-t border-border/40 bg-background">
	<Container class="flex flex-col md:flex-row items-center justify-between gap-6">
		<div class="flex items-center gap-2">
			<div class="size-5 rounded bg-primary flex items-center justify-center">
				<div class="size-2 rounded-full border border-primary-foreground"></div>
			</div>
			<span class="font-semibold tracking-tight">Recast</span>
		</div>
		<p class="text-sm text-muted-foreground font-medium">Built for clarity. Designed for speed.</p>
		<p class="text-sm text-muted-foreground">© 2026 Recast. All rights reserved.</p>
	</Container>
</footer>