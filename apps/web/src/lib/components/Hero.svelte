<script lang="ts">
	import { Button } from "@recast/ui/button";
	import { Container, Section } from ".";
</script>

<Section class="relative pt-32 pb-16 md:pt-40 md:pb-24">
	<div class="absolute inset-0 bg-grid-pattern opacity-40 pointer-events-none"></div>

	<Container class="relative z-10">
		<div class="max-w-4xl">
			<h1 class="text-6xl md:text-8xl lg:text-9xl font-semibold tracking-[-0.03em] mb-10 text-foreground">
				Video editing, <br />
				<span class="text-foreground/40 font-medium italic">refined.</span>
			</h1>
			
			<p class="max-w-2xl text-xl md:text-2xl text-foreground/50 leading-relaxed mb-16 font-medium">
				Recast is the fast, minimal, and intentional editor built for the next generation of storytellers. Smooth by design, powerful by default.
			</p>
			
			<div class="flex flex-wrap items-center gap-6">
				<Button size="lg" class="h-14 px-10 text-[15px] font-semibold bg-foreground text-background hover:bg-foreground/90 rounded-2xl shadow-craft-md transition-all active:scale-95">
					Try for Free
				</Button>
				<Button variant="ghost" size="lg" class="h-14 px-10 text-[15px] font-semibold text-foreground/60 hover:text-foreground rounded-2xl transition-all">
					Watch Demo
				</Button>
			</div>
		</div>

		<div class="mt-24 md:mt-32 relative group">
			<div class="craft-card overflow-hidden shadow-craft-xl transition-all duration-700 group-hover:scale-[1.005]">
				<div class="h-10 border-b border-border-low bg-white/50 dark:bg-black/20 flex items-center px-4 gap-2">
					<div class="flex gap-1.5">
						<div class="size-2.5 rounded-full bg-border-low"></div>
						<div class="size-2.5 rounded-full bg-border-low"></div>
						<div class="size-2.5 rounded-full bg-border-low"></div>
					</div>
				</div>
				<img src="/product_preview_hero.png" alt="Recast Product Preview" class="w-full h-auto opacity-90 transition-opacity group-hover:opacity-100" />
			</div>
			
			<div class="absolute -top-12 -right-12 md:top-8 md:-right-24 p-4 craft-card invisible-ui rotate-3">
				<div class="flex items-center gap-3 text-sm font-medium">
					<div class="size-2 rounded-full bg-green-500 animate-pulse"></div>
					Perfect motion smoothing
				</div>
			</div>
		</div>
	</Container>
</Section>
