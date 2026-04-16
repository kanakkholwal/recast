<script lang="ts">
	import { MousePointer2, Layout, Scissors, WifiOff, FileBox, Zap } from "lucide-svelte";
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
			description: "No uploads required. Everything runs locally.",
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

<Section id="features" class="py-32 bg-background">
	<Container>
		<div class="mb-24">
			<h2 class="text-5xl font-semibold tracking-tight mb-6 text-foreground">Core Features.</h2>
			<p class="text-xl text-foreground/50">Everything you need. Nothing you don't.</p>
		</div>

		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
			{#each features as feature}
				<div class="group craft-block hover:bg-secondary dark:hover:bg-white/[0.02] border border-border-low shadow-craft-sm">
					<div class="size-10 rounded-xl bg-background border border-border-low flex items-center justify-center mb-6 group-hover:scale-110 transition-transform">
						{#if feature.icon}
							{@const Icon = feature.icon}
							<Icon class="size-4 text-foreground/60" />
						{/if}
					</div>
					<h3 class="text-lg font-semibold mb-3 text-foreground">{feature.title}</h3>
					<p class="text-sm text-foreground/50 leading-relaxed font-medium">
						{feature.description}
					</p>
					
					<div class="mt-6 flex items-center gap-2 text-[11px] font-bold uppercase tracking-widest text-foreground/20 invisible-ui">
						Learn more
					</div>
				</div>
			{/each}
		</div>
	</Container>
</Section>