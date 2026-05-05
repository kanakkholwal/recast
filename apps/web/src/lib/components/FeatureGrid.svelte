<script lang="ts">
	import { Container, Reveal, Section, SectionHeader } from "$lib/components";
	import { cn } from "@recast/ui/utils";
	import {
		FileBox,
		Layout,
		MousePointer2,
		Scissors,
		WifiOff,
		Zap,
		type Icon as LucideIcon,
	} from "lucide-svelte";

	type Feature = {
		title: string;
		description: string;
		icon: typeof LucideIcon;
		span: string;
		accent?: boolean;
	};

	const features: Feature[] = [
		{
			title: "Cursor engine",
			description:
				"Velocity-based smoothing and snap targeting. Cursor paths feel intentional, never twitchy.",
			icon: MousePointer2,
			span: "md:col-span-4 md:row-span-2",
			accent: true,
		},
		{
			title: "Smart layouts",
			description: "Auto padding, backgrounds, and framing applied in real time.",
			icon: Layout,
			span: "md:col-span-2",
		},
		{
			title: "Lightweight editor",
			description: "Trim, tweak, ship. No timeline gymnastics.",
			icon: Scissors,
			span: "md:col-span-2",
		},
		{
			title: "Offline first",
			description:
				"Recordings, projects, and exports stay on your machine. No accounts, no uploads.",
			icon: WifiOff,
			span: "md:col-span-3",
		},
		{
			title: "Project files (.recast)",
			description: "Re-editable project artifacts that travel with your repo.",
			icon: FileBox,
			span: "md:col-span-3",
		},
		{
			title: "Fast export",
			description: "GPU-accelerated rendering pipeline. Seconds, not minutes.",
			icon: Zap,
			span: "md:col-span-6",
		},
	];
</script>

<Section id="features" class="border-t border-border-low/60">
	<Container>
		<SectionHeader
			eyebrow="Core engine"
			title="Everything you need. Nothing you don't."
			description="Recast is a focused recorder, not a kitchen-sink editor. Six primitives, deeply tuned."
		/>

		<div class="mt-16 grid grid-cols-1 gap-3 md:grid-cols-6 md:auto-rows-[minmax(180px,1fr)] md:gap-4">
			{#each features as feature, i}
				{@const Icon = feature.icon}
				<Reveal delay={i * 60} class={cn("group", feature.span)}>
					<article
						class={cn(
							"glass-card relative flex h-full flex-col justify-between overflow-hidden rounded-2xl p-6 transition-all duration-300 hover:-translate-y-0.5 hover:shadow-craft-md",
							feature.accent && "bg-linear-to-br from-primary/10 via-transparent to-transparent",
						)}
					>
						<div
							class="pointer-events-none absolute -right-12 -top-12 size-40 rounded-full bg-primary/5 blur-3xl transition-opacity duration-500 group-hover:bg-primary/12"
						></div>

						<div class="relative flex items-start justify-between">
							<span
								class={cn(
									"glass-chip grid size-10 place-items-center rounded-xl text-foreground/70 transition-all duration-300 group-hover:scale-105 group-hover:text-primary",
									feature.accent && "text-primary",
								)}
							>
								<Icon class="size-5" />
							</span>
						</div>

						<div class="relative mt-12">
							<h3 class="text-lg font-semibold tracking-tight text-foreground">
								{feature.title}
							</h3>
							<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
								{feature.description}
							</p>
						</div>
					</article>
				</Reveal>
			{/each}
		</div>
	</Container>
</Section>
