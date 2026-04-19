<script lang="ts">
	import { Button } from "@recast/ui/button";
	import { Container, Section } from ".";

	let words = ["recorder.","creator.", "refiner.", "generator.", "editor."];
	let currentWordIndex = $state(0);
	let mounted = $state(false);

	$effect(() => {
		mounted = true;
		const interval = setInterval(() => {
			currentWordIndex = (currentWordIndex + 1) % words.length;
		}, 2500);

		return () => clearInterval(interval);
	});
</script>

<Section class="relative pt-32 pb-16 md:pt-40 md:pb-24 overflow-hidden">
	<div class="absolute inset-0 bg-grid-pattern opacity-40 pointer-events-none"></div>

	<Container class="relative z-10">
		<div class="max-w-6xl">
			<!-- Main Heading -->
			<div class="transform transition-all duration-1000 ease-out {mounted ? 'translate-y-0 opacity-100' : 'translate-y-8 opacity-0'}">
				<h1 class="text-6xl md:text-7xl lg:text-8xl font-semibold tracking-[-0.03em] mb-8 text-foreground leading-[1.15]">
					The fastest product <br /> demo 
					<span class="inline-grid overflow-hidden align-bottom text-foreground/40 font-medium italic">
						{#each words as word, i}
							<span 
								class="col-start-1 row-start-1 transition-all duration-500 ease-in-out"
								style="transform: translateY({(i - currentWordIndex) * 100}%); opacity: {i === currentWordIndex ? 1 : 0};"
							>
								{word}
							</span>
						{/each}
					</span>
				</h1>
			</div>

			<!-- Subheading -->
			<div class="transform transition-all duration-1000 delay-200 ease-out {mounted ? 'translate-y-0 opacity-100' : 'translate-y-8 opacity-0'}">
				<p class="max-w-2xl text-lg md:text-xl text-foreground/50 leading-relaxed mb-12 font-medium font-serif">
					Stop wrestling with timeline tools. Recast seamlessly records, refines, and generates polished, startup-ready walkthroughs with cinematic magic.
				</p>
			</div>

			<!-- CTAs -->
			<div class="flex flex-wrap items-center gap-6 transform transition-all duration-1000 delay-300 ease-out {mounted ? 'translate-y-0 opacity-100' : 'translate-y-8 opacity-0'}">
				<a href="/download">
					<Button size="lg" class="h-14 px-10 text-[15px] font-semibold bg-foreground text-background hover:bg-foreground/90 rounded-2xl shadow-craft-md transition-all active:scale-95 duration-200">
						Download Recast
					</Button>
				</a>
				<Button variant="ghost" size="lg" class="h-14 px-10 text-[15px] font-semibold text-foreground/60 hover:text-foreground rounded-2xl transition-all duration-200">
					Watch Demo
				</Button>
			</div>
		</div>

		<!-- Dashboard Preview -->
		<div class="mt-20 md:mt-28 relative group transform transition-all duration-[1200ms] delay-500 ease-out {mounted ? 'translate-y-0 opacity-100' : 'translate-y-12 opacity-0'}">
			<div class="craft-card overflow-hidden shadow-craft-xl transition-all duration-700 group-hover:scale-[1.005]">
				<div class="h-10 border-b border-border-low bg-white/50 dark:bg-black/20 flex items-center px-4 gap-2">
					<div class="flex gap-1.5">
						<div class="size-2.5 rounded-full bg-border-low transition-colors group-hover:bg-red-500/80"></div>
						<div class="size-2.5 rounded-full bg-border-low transition-colors group-hover:bg-yellow-500/80"></div>
						<div class="size-2.5 rounded-full bg-border-low transition-colors group-hover:bg-green-500/80"></div>
					</div>
				</div>
				<img src="/product_preview_hero.png" alt="Recast Product Preview" class="w-full h-auto opacity-90 transition-opacity duration-500 group-hover:opacity-100 block" />
			</div>

			<div class="absolute -top-12 -right-12 md:top-8 md:-right-24 p-4 craft-card invisible-ui rotate-3">
				<div class="flex items-center gap-3 text-sm font-medium pr-1">
					<div class="relative flex h-3 w-3">
						<span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
						<span class="relative inline-flex rounded-full h-3 w-3 bg-green-500"></span>
					</div>
					<span>Cinematic tracking & zoom</span>
				</div>
			</div>
		</div>
	</Container>
</Section>
