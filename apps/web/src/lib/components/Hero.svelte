<script lang="ts">
	import { Container, Eyebrow, Section } from "$lib/components";
	import { TextLoop } from "$lib/motion-core";
	import { Button } from "@recast/ui/button";
	import { ArrowRight, Download, Sparkles } from "lucide-svelte";

	const words = [
		"expensive.",
		"cinematic.",
		"intentional.",
		"effortless.",
		"hand-edited.",
	];
	const platforms = ["macOS", "Windows", "Linux"];
</script>

<Section spacing="none" class="hero-atmosphere relative overflow-hidden pt-36 pb-20 md:pt-44 md:pb-28">
	<div aria-hidden="true" class="hero-aurora pointer-events-none absolute inset-0 -z-10"></div>
	<div aria-hidden="true" class="hero-clouds pointer-events-none absolute inset-x-0 top-0 -z-10 h-160"></div>
	<div aria-hidden="true" class="hero-grid pointer-events-none absolute inset-0 -z-10 opacity-[0.35]"></div>

	<Container class="relative">
		<div class="mx-auto flex max-w-6xl flex-col items-center text-center">
			<a href="/changelog" class="group inline-block animate-fade-in">
				<Eyebrow icon={Sparkles} variant="primary">
					<span>what's new</span>
					<ArrowRight class="size-3 transition-transform group-hover:translate-x-0.5" />
				</Eyebrow>
			</a>

			<h1
				class="text-balance mt-7 animate-fade-up text-5xl font-semibold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5.25rem]"
			>
				Demos that look
				<span class="mt-2 flex justify-center font-medium italic text-foreground/40">
					<span class="inline-grid overflow-hidden">
						<TextLoop class="text-primary" texts={words} interval={3000} />
					</span>
				</span>
			</h1>

			<p
				class="text-pretty mt-7 max-w-2xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg md:text-xl"
				style="animation-delay: 120ms"
			>
				The native screen recorder for founders, indie hackers, and product engineers who'd rather ship than open a timeline.
			</p>

			<div
				class="mt-10 flex animate-fade-up flex-col items-center gap-3 sm:flex-row sm:gap-4"
				style="animation-delay: 240ms"
			>
				<Button href="/download" size="lg" class="gap-2.5">
					<Download class="size-4" />
					Get started
				</Button>
				<Button href="/features" variant="outline" size="lg" class="group/cta gap-2">
					See a demo
					<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
				</Button>
			</div>

			<div
				class="mt-8 flex animate-fade-up items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground/80"
				style="animation-delay: 360ms"
			>
				<span class="relative flex size-1.5">
					<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-70"></span>
					<span class="relative inline-flex size-1.5 rounded-full bg-primary"></span>
				</span>
				Free during beta · No sign-up
				<span class="mx-2 hidden h-1 w-1 rounded-full bg-muted-foreground/40 sm:inline-block"></span>
				<span class="hidden items-center gap-2 sm:inline-flex">
					{#each platforms as p, i}
						<span>{p}</span>
						{#if i < platforms.length - 1}
							<span class="text-muted-foreground/40">·</span>
						{/if}
					{/each}
				</span>
			</div>
		</div>

		<div
			class="relative mx-auto mt-20 max-w-6xl animate-fade-up"
			style="animation-delay: 460ms"
		>
			<div class="glass-card group/preview relative overflow-hidden rounded-2xl shadow-craft-xl ring-1 ring-foreground/5">
				<div class="flex h-10 items-center gap-2 border-b border-border-low/40 bg-white/5 px-4 dark:bg-white/3">
					<div class="flex gap-1.5">
						<span class="size-2.5 rounded-full bg-foreground/15 transition-colors group-hover/preview:bg-destructive/70"></span>
						<span class="size-2.5 rounded-full bg-foreground/15 transition-colors group-hover/preview:bg-warning/70"></span>
						<span class="size-2.5 rounded-full bg-foreground/15 transition-colors group-hover/preview:bg-success/70"></span>
					</div>
					<div class="ml-3 flex items-center gap-2 text-[11px] font-medium text-muted-foreground">
						<span class="hidden sm:inline">recast.nexonauts.com</span>
						<span class="hidden sm:inline">·</span>
						<span>Untitled recording</span>
					</div>
				</div>
				<div class="bg-linear-to-b from-muted/10 to-background p-1.5 sm:p-2">
					<img
						src="/product_preview_hero.png"
						alt="Recast app preview"
						loading="eager"
						decoding="async"
						class="block w-full rounded-xl object-cover ring-1 ring-border-low"
					/>
				</div>
			</div>

			<div class="glass-chip absolute -bottom-4 left-4 hidden items-center gap-2.5 rounded-xl px-3.5 py-2 shadow-craft-floating sm:flex md:-bottom-5 md:left-8">
				<span class="relative flex size-2">
					<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/50"></span>
					<span class="relative inline-flex size-2 rounded-full bg-primary"></span>
				</span>
				<span class="text-xs font-semibold text-foreground">Recording · 00:42</span>
			</div>

			<div class="glass-chip absolute -top-4 right-4 hidden items-center gap-2 rounded-xl px-3.5 py-2 shadow-craft-floating sm:flex md:-top-5 md:right-8">
				<span class="text-xs font-semibold text-foreground">Cursor smoothed</span>
				<span class="rounded-md bg-primary/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-primary">
					Auto
				</span>
			</div>
		</div>
	</Container>
</Section>

<style>
	.hero-aurora {
		background:
			radial-gradient(ellipse 80% 50% at 50% -10%, color-mix(in srgb, var(--color-primary) 12%, transparent), transparent 70%),
			radial-gradient(ellipse 60% 40% at 18% 8%, color-mix(in srgb, var(--color-primary) 7%, transparent), transparent 70%),
			radial-gradient(ellipse 60% 40% at 82% 8%, color-mix(in srgb, var(--color-primary) 8%, transparent), transparent 70%);
	}

	:global(.dark) .hero-aurora {
		background:
			radial-gradient(ellipse 80% 50% at 50% -10%, color-mix(in srgb, var(--color-primary) 7%, transparent), transparent 75%),
			radial-gradient(ellipse 60% 40% at 18% 8%, color-mix(in srgb, var(--color-primary) 4%, transparent), transparent 75%),
			radial-gradient(ellipse 60% 40% at 82% 8%, color-mix(in srgb, var(--color-primary) 5%, transparent), transparent 75%);
	}

	.hero-clouds {
		background:
			radial-gradient(ellipse 40% 20% at 25% 30%, color-mix(in srgb, white 18%, transparent), transparent 75%),
			radial-gradient(ellipse 35% 18% at 75% 18%, color-mix(in srgb, white 14%, transparent), transparent 75%);
		filter: blur(50px);
		opacity: 0.5;
	}

	:global(.dark) .hero-clouds {
		background:
			radial-gradient(ellipse 40% 20% at 25% 30%, color-mix(in srgb, var(--color-foreground) 5%, transparent), transparent 75%),
			radial-gradient(ellipse 35% 18% at 75% 18%, color-mix(in srgb, var(--color-foreground) 4%, transparent), transparent 75%);
		filter: blur(50px);
		opacity: 0.4;
	}

	.hero-grid {
		background-image:
			linear-gradient(to right, color-mix(in srgb, var(--color-foreground) 5%, transparent) 1px, transparent 1px),
			linear-gradient(to bottom, color-mix(in srgb, var(--color-foreground) 5%, transparent) 1px, transparent 1px);
		background-size: 64px 64px;
		mask-image: radial-gradient(ellipse 70% 60% at 50% 30%, black 30%, transparent 75%);
	}
</style>
