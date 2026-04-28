<script lang="ts">
	import {
		Container,
		FeatureGrid,
		Footer,
		Hero,
		Section,
	} from "$lib/components";
	import { Button } from "@recast/ui/button";
	import {
		Download,
		Play,
		Rocket,
		Terminal,
		Users,
		Wand2,
	} from "lucide-svelte";
	import { spring, type Spring } from "svelte/motion";

	let prefersReducedMotion = $state(false);

	const philosophyAnim = spring(0, { stiffness: 0.04, damping: 0.6 });
	const useCasesAnim = spring(0, { stiffness: 0.04, damping: 0.6 });

	function viewport(element: HTMLElement, store: Spring<number>) {
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries[0].isIntersecting) {
					store.set(1);
					observer.disconnect();
				}
			},
			{ threshold: 0.15 },
		);
		observer.observe(element);
		return {
			destroy() {
				observer.disconnect();
			},
		};
	}

	$effect(() => {
		const mediaQuery = window.matchMedia(
			"(prefers-reduced-motion: reduce)",
		);
		prefersReducedMotion = mediaQuery.matches;
		const update = (e: MediaQueryListEvent) =>
			(prefersReducedMotion = e.matches);
		mediaQuery.addEventListener("change", update);
		return () => mediaQuery.removeEventListener("change", update);
	});
</script>

<svelte:head>
	<title>Recast — The Native Screen Recorder</title>
	<meta
		name="description"
		content="Recast captures your screen with cinematic cursor motion, auto-enhanced visuals, and zero-lag export."
	/>
</svelte:head>

<main class="bg-background text-foreground selection:bg-primary/10 font-sans">
	<Hero />

	<!-- PHILOSOPHY / WORKFLOW SECTION -->
	<Section
		id="workflow"
		class="py-24 md:py-40 relative z-10 bg-linear-to-b from-background to-muted/20 border-border-low/40"
	>
		<Container>
			<div
				class="grid grid-cols-1 lg:grid-cols-12 gap-16 lg:gap-24 items-start"
				use:viewport={philosophyAnim}
			>
				<div
					class="lg:col-span-5 sticky top-32"
					style="opacity: {$philosophyAnim}; transform: translateY({prefersReducedMotion
						? 0
						: (1 - $philosophyAnim) * 20}px);"
				>
					<div
						class="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-border-low bg-muted/30 backdrop-blur-md mb-8 shadow-craft-sm"
					>
						<Wand2 class="size-3 text-foreground/70" />
						<span
							class="text-[11px] font-bold uppercase tracking-[0.15em] text-foreground/60"
							>Crafted Workflow</span
						>
					</div>
					<h2
						class="text-4xl md:text-5xl lg:text-[3.5rem] font-semibold mb-8 text-foreground tracking-tight leading-[1.1]"
					>
						Designed for <br />
						<span class="text-foreground/40 italic font-serif"
							>clarity.</span
						>
					</h2>
					<p
						class="text-xl text-foreground/60 leading-relaxed max-w-md font-medium"
					>
						Raw capture is just the beginning. Recast refines your
						output instantly, adding framing, smoothing, and layout
						polish in zero clicks.
					</p>
				</div>

				<div class="lg:col-span-7 space-y-8">
					{#each [{ step: "01", icon: Play, title: "Native Recording", desc: "No proxy layers. Capture any window or screen natively at full framerate with imperceptible overhead." }, { step: "02", icon: Wand2, title: "Cinematic Polish", desc: "Instantly apply velocity-based cursor smoothing, smart padding, and clean layout backgrounds." }, { step: "03", icon: Download, title: "Instant Export", desc: "No timeline rendering. Hardware-accelerated exports finish in seconds, or save as a native .recast project." }] as { step, icon: Icon, title, desc }, i}
						<div
							class="group relative overflow-hidden bg-background/40 backdrop-blur-xl border border-border-low p-8 transition-all duration-300 hover:bg-muted/30 shadow-craft-sm hover:shadow-craft-md hover:-translate-y-1 invisible-ui"
							style="opacity: {Math.max(
								0,
								($philosophyAnim - i * 0.15) * 1.25,
							)}; transform: translateY({prefersReducedMotion
								? 0
								: (1 -
										Math.max(
											0,
											($philosophyAnim - i * 0.15) * 1.25,
										)) *
									20}px);"
						>
							<div class="flex items-start gap-6">
								<div
									class="mt-1 flex-shrink-0 size-12 rounded-2xl bg-muted border border-border flex items-center justify-center text-foreground/60 group-hover:bg-primary/5 group-hover:text-primary transition-colors"
								>
									<Icon class="size-5" />
								</div>
								<div>
									<div
										class="text-[12px] font-bold text-foreground/30 mb-2 uppercase tracking-wider"
									>
										{step}
									</div>
									<h4
										class="text-2xl font-semibold mb-3 text-foreground tracking-tight"
									>
										{title}
									</h4>
									<p
										class="text-foreground/60 leading-relaxed text-[16px]"
									>
										{desc}
									</p>
								</div>
							</div>
						</div>
					{/each}
				</div>
			</div>
		</Container>
	</Section>

	<FeatureGrid />

	<!-- USE CASES -->
	<Section id="use-cases" class="py-32">
		<Container>
			<div
				class="text-center mb-24 flex flex-col items-center"
				use:viewport={useCasesAnim}
				style="opacity: {$useCasesAnim}; transform: translateY({prefersReducedMotion
					? 0
					: (1 - $useCasesAnim) * 20}px);"
			>
				<h2
					class="text-4xl md:text-6xl font-semibold mb-6 text-foreground tracking-tight"
				>
					Purpose-built.
				</h2>
				<p class="text-xl text-foreground/50 max-w-2xl font-medium">
					Not a bloated editor. Not a laggy cloud recorder. Built
					precisely for high-fidelity communication.
				</p>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
				{#each [{ icon: Terminal, title: "Engineers", desc: "Demo PRs, explain complex bugs, and document APIs cleanly without writing a novel." }, { icon: Rocket, title: "Founders", desc: "Create pixel-perfect product walkthroughs that impress investors and users." }, { icon: Users, title: "Teams", desc: "Replace asynchronous chaos with crisp, clear, visual updates that actually get watched." }] as item, i}
					{@const Icon = item.icon}
					<div
						class="group relative overflow-hidden bg-background border border-border-low rounded-[2rem] p-8 shadow-craft-sm hover:shadow-craft-lg transition-all duration-300 hover:-translate-y-1 invisible-ui"
						style="opacity: {Math.max(
							0,
							($useCasesAnim - i * 0.1) * 1.25,
						)}; transform: translateY({prefersReducedMotion
							? 0
							: (1 -
									Math.max(
										0,
										($useCasesAnim - i * 0.1) * 1.25,
									)) *
								30}px);"
					>
						<div
							class="absolute -right-8 -top-8 size-32 bg-primary/5 rounded-full blur-3xl group-hover:bg-primary/10 transition-colors duration-500"
						></div>
						<div
							class="size-12 rounded-xl bg-muted border border-border flex items-center justify-center mb-10 group-hover:scale-[1.05] group-hover:bg-primary/5 transition-all duration-300"
						>
							<Icon
								class="size-5 text-foreground/70 group-hover:text-primary transition-colors"
							/>
						</div>
						<h4
							class="text-xl font-semibold mb-3 text-foreground tracking-tight"
						>
							{item.title}
						</h4>
						<p
							class="text-foreground/60 leading-relaxed text-[15px]"
						>
							{item.desc}
						</p>
					</div>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- CTA Section -->
	<Section
		id="cta"
		class="py-24 md:py-32 bg-background border-t border-border-low"
	>
		<Container>
			<div
				class="craft-card p-12 md:p-24 text-center relative overflow-hidden group bg-linear-to-b from-muted/10 to-background rounded-[3rem] border border-border-low shadow-craft-lg"
			>
				<div
					class="absolute inset-0 bg-grid-pattern opacity-10 group-hover:opacity-20 transition-opacity duration-1000 mix-blend-overlay"
				></div>
				<div
					class="relative z-10 max-w-2xl mx-auto flex flex-col items-center"
				>
					<h2
						class="text-4xl md:text-6xl font-semibold mb-8 text-foreground tracking-tight leading-[1.1]"
					>
						Ready to refine your workflow?
					</h2>
					<Button
						size="lg"
						href="/download"
						class="h-14 px-10 text-[16px] font-bold bg-foreground text-background hover:scale-[1.02] active:scale-95 transition-all rounded-2xl shadow-craft-xl duration-300"
					>
						Download Recast
					</Button>
					<p
						class="mt-6 text-[13px] font-semibold tracking-wide uppercase text-foreground/40"
					>
						Free during beta. Windows only for now.
					</p>
				</div>
			</div>
		</Container>
	</Section>
</main>

<Footer />
