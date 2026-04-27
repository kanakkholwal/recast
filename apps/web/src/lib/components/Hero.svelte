<script lang="ts">
	import { Button } from "@recast/ui/button";
	import { fly } from "svelte/transition";
	import { Container, Section } from ".";

	let words = ["recorder.", "creator.", "refiner.", "generator.", "editor."];
	let currentWordIndex = $state(0);
	let mounted = $state(false);
	let prefersReducedMotion = $state(false);

	$effect(() => {
		mounted = true;

		const mediaQuery = window.matchMedia(
			"(prefers-reduced-motion: reduce)",
		);
		prefersReducedMotion = mediaQuery.matches;
		const update = (e: MediaQueryListEvent) =>
			(prefersReducedMotion = e.matches);
		mediaQuery.addEventListener("change", update);

		const interval = setInterval(() => {
			currentWordIndex = (currentWordIndex + 1) % words.length;
		}, 3000);

		return () => {
			clearInterval(interval);
			mediaQuery.removeEventListener("change", update);
		};
	});
</script>

<Section class="relative pt-32 pb-16 md:pt-48 md:pb-32 overflow-hidden">
	<div
		class="absolute inset-0 bg-grid-pattern opacity-[0.12] pointer-events-none mix-blend-overlay"
	></div>

	<Container class="relative z-10">
		<div class="max-w-5xl mx-auto text-center flex flex-col items-center">
			{#if mounted}
				<div
					class="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-border-low bg-muted/30 backdrop-blur-md mb-10 shadow-craft-sm"
					in:fly={{
						y: prefersReducedMotion ? 0 : -10,
						duration: 800,
						opacity: 0,
					}}
				>
					<div class="size-2 rounded-full bg-foreground/20"></div>
					<span
						class="text-[11px] font-bold uppercase tracking-[0.15em] text-foreground/60"
						>Recast Desktop Beta</span
					>
				</div>

				<h1
					class="text-5xl md:text-7xl lg:text-[5.5rem] font-semibold tracking-tight mb-8 text-foreground leading-[1.05] flex flex-wrap justify-center gap-x-4"
					in:fly={{
						y: prefersReducedMotion ? 0 : 20,
						duration: 1000,
						delay: 100,
						opacity: 0,
					}}
				>
					<span>The native</span>
					<div
						class="relative inline-grid items-center h-[1.15em] overflow-hidden text-foreground/40 font-serif italic pr-2"
					>
						{#each words as word, i (word)}
							{#if i === currentWordIndex}
								<span
									class="col-start-1 row-start-1"
									in:fly={{
										y: prefersReducedMotion ? 0 : 40,
										duration: 800,
										delay: 150,
									}}
									out:fly={{
										y: prefersReducedMotion ? 0 : -40,
										duration: 800,
									}}
								>
									{word}
								</span>
							{/if}
						{/each}
					</div>
				</h1>

				<p
					class="max-w-2xl text-lg md:text-xl text-foreground/50 leading-relaxed mb-12 font-medium"
					in:fly={{
						y: prefersReducedMotion ? 0 : 20,
						duration: 1000,
						delay: 200,
						opacity: 0,
					}}
				>
					Stop wrestling with timeline tools. Recast seamlessly
					captures and refines your workflow into polished,
					startup-ready walkthroughs with cinematic magic.
				</p>

				<div
					class="flex flex-wrap items-center justify-center gap-4"
					in:fly={{
						y: prefersReducedMotion ? 0 : 20,
						duration: 1000,
						delay: 300,
						opacity: 0,
					}}
				>
					<a href="/download">
						<Button
							size="lg"
							class="h-14 px-8 text-[15px] font-semibold bg-foreground text-background hover:bg-foreground/90 rounded-2xl shadow-craft-md transition-all active:scale-95 duration-200"
						>
							Download for Mac
						</Button>
					</a>
					<Button
						variant="ghost"
						size="lg"
						class="h-14 px-8 text-[15px] font-semibold text-foreground/60 hover:text-foreground rounded-2xl transition-colors duration-200"
					>
						View Documentation
					</Button>
				</div>
			{/if}
		</div>

		{#if mounted}
			<div
				class="mt-20 md:mt-32 relative group mx-auto max-w-5xl"
				in:fly={{
					y: prefersReducedMotion ? 0 : 40,
					duration: 1200,
					delay: 450,
					opacity: 0,
				}}
			>
				<div
					class="craft-card overflow-hidden shadow-craft-xl rounded-[2rem] border border-border/40 bg-background/50 backdrop-blur-3xl ring-1 ring-foreground/5"
				>
					<div
						class="h-12 border-b border-border-low bg-muted/20 flex items-center px-6 gap-2"
					>
						<div class="flex gap-2">
							<div
								class="size-3 rounded-full bg-border-low transition-colors group-hover:bg-destructive/80"
							></div>
							<div
								class="size-3 rounded-full bg-border-low transition-colors group-hover:bg-yellow-500/80"
							></div>
							<div
								class="size-3 rounded-full bg-border-low transition-colors group-hover:bg-green-500/80"
							></div>
						</div>
					</div>
					<div class="p-2 bg-linear-to-b from-muted/5 to-background">
						<img
							src="/product_preview_hero.png"
							alt="Recast Product Preview"
							class="w-full h-auto rounded-xl ring-1 ring-border-low shadow-sm opacity-95 transition-opacity duration-500 group-hover:opacity-100 block object-cover"
						/>
					</div>
				</div>

				<div
					class="absolute -right-2 top-1/4 md:-right-8 md:top-1/3 p-4 craft-card rounded-2xl shadow-craft-floating invisible-ui transition-all duration-500 translate-x-4 group-hover:translate-x-0"
				>
					<div
						class="flex items-center gap-3 text-sm font-semibold text-foreground/80"
					>
						<div class="relative flex h-2.5 w-2.5">
							<span
								class="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-50"
							></span>
							<span
								class="relative inline-flex rounded-full h-2.5 w-2.5 bg-primary"
							></span>
						</div>
						<span>Zero-lag processing</span>
					</div>
				</div>
			</div>
		{/if}
	</Container>
</Section>
