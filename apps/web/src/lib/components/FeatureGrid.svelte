<script lang="ts">
	import { MousePointer2, Layout, Scissors, WifiOff, FileBox, Zap, ArrowUpRight } from "lucide-svelte";
	import { Container, Section } from ".";

	const features = [
		{
			title: "Cursor Engine",
			description: "Precision tracking with smooth interpolation.",
			icon: MousePointer2,
		},
		{
			title: "Smart Layouts",
			description: "Auto padding, backgrounds, and framing.",
			icon: Layout,
		},
		{
			title: "Lightweight Editor",
			description: "Trim, tweak, done.",
			icon: Scissors,
		},
		{
			title: "Offline First",
			description: "No uploads required. runs locally.",
			icon: WifiOff,
		},
		{
			title: "Project Files (.recast)",
			description: "Edit anytime without losing data.",
			icon: FileBox,
		},
		{
			title: "Fast Export",
			description: "GPU-powered rendering pipeline.",
			icon: Zap,
		},
	];
</script>

<Section id="features" class="py-24 md:py-32 bg-background relative border-t border-border-low/50">
	<Container>
		<div class="mb-20">
			<h2 class="text-4xl md:text-5xl font-semibold tracking-tight mb-6 text-foreground">Core engine.</h2>
			<p class="text-xl text-foreground/50 font-medium">Everything you need. Nothing you don't.</p>
		</div>

		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
			{#each features as feature}
				<div class="group relative overflow-hidden bg-background border border-border-low rounded-[2rem] p-8 shadow-craft-sm hover:shadow-craft-md transition-all duration-300 hover:-translate-y-1 invisible-ui">
					<div class="absolute -right-12 -top-12 size-32 bg-foreground/5 rounded-full blur-3xl group-hover:bg-primary/5 transition-colors duration-500"></div>
					
					<div class="size-12 rounded-2xl bg-muted border border-border flex items-center justify-center mb-8 group-hover:scale-105 group-hover:bg-primary/5 transition-all duration-300 shadow-sm">
						{#if feature.icon}
							{@const Icon = feature.icon}
							<Icon class="size-5 text-foreground/60 group-hover:text-primary transition-colors" />
						{/if}
					</div>
					
					<div>
						<h3 class="text-xl font-semibold mb-3 text-foreground tracking-tight">{feature.title}</h3>
						<p class="text-base text-foreground/50 leading-relaxed font-medium">
							{feature.description}
						</p>
					</div>
					
					<div class="mt-8 flex items-center justify-between opacity-0 group-hover:opacity-100 transition-opacity duration-300">
						<div class="text-[11px] font-bold uppercase tracking-widest text-primary/80">
							Explore
						</div>
						<div class="size-6 rounded-full bg-primary/10 flex items-center justify-center text-primary">
							<ArrowUpRight class="size-3" />
						</div>
					</div>
				</div>
			{/each}
		</div>
	</Container>
</Section>