<script lang="ts">
	import { Button } from "@recast/ui/button";
	import { Container, Section } from ".";
</script>

<Section class="relative overflow-hidden pt-36 pb-20 md:pt-48 md:pb-32">
	<div class="absolute top-0 left-1/2 -translate-x-1/2 w-full max-w-7xl h-full -z-10 opacity-30 pointer-events-none">
		<div class="absolute top-0 left-1/4 w-96 h-96 bg-primary/20 rounded-full blur-[120px]"></div>
		<div class="absolute bottom-0 right-1/4 w-96 h-96 bg-secondary/10 rounded-full blur-[120px]"></div>
	</div>

	<Container class="text-center">
		<h1 class="text-5xl md:text-7xl lg:text-8xl font-bold tracking-tight mb-8 animate-in fade-in slide-in-from-bottom-8 duration-700">
			Video editing, <br />
			<span class="text-primary italic">refined.</span>
		</h1>
		<p class="max-w-2xl mx-auto text-lg md:text-xl text-muted-foreground mb-12 animate-in fade-in slide-in-from-bottom-8 duration-700 delay-100">
			Recast is the fast, minimal, and intentional editor built for the next generation of storytellers. Smooth by design, powerful by default.
		</p>
		<div class="flex flex-col sm:flex-row items-center justify-center gap-4 animate-in fade-in slide-in-from-bottom-8 duration-700 delay-200">
			<Button size="lg" class="w-full sm:w-auto px-8 py-6 text-base shadow-lg shadow-primary/20">Try for Free</Button>
			<Button variant="outline" size="lg" class="w-full sm:w-auto px-8 py-6 text-base">Watch Demo</Button>
		</div>

		<div class="mt-20 md:mt-32 relative rounded-2xl border border-border/50 bg-card shadow-2xl overflow-hidden animate-in fade-in zoom-in duration-1000 delay-300 mx-auto max-w-5xl group">
			<div class="absolute inset-0 bg-linear-to-tr from-primary/10 via-transparent to-secondary/10 opacity-0 group-hover:opacity-100 transition-opacity duration-700"></div>
			<img src="/product_preview_hero.png" alt="Recast Product Preview" class="w-full h-auto" />
			<div class="absolute inset-0 pointer-events-none border border-white/10 rounded-2xl"></div>
		</div>
	</Container>
</Section>
