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

<Section id="features" class="py-24 bg-background">
	<Container>
		<div class="mb-16">
			<h2 class="text-3xl font-semibold tracking-tight mb-4">Core Features</h2>
			<p class="text-muted-foreground text-lg">Everything you need. Nothing you don't.</p>
		</div>

		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
			{#each features as feature}
				<div class="p-6 rounded-2xl border border-border/50 bg-background hover:bg-muted/20 transition-colors duration-200">
					<div class="size-10 rounded-lg bg-muted flex items-center justify-center mb-5">
						{#if feature.icon}
							{@const Icon = feature.icon}
							<Icon class="size-5 text-foreground" />
						{/if}
					</div>
					<h3 class="text-lg font-semibold mb-2">{feature.title}</h3>
					<p class="text-sm text-muted-foreground leading-relaxed">
						{feature.description}
					</p>
				</div>
			{/each}
		</div>
	</Container>
</Section>