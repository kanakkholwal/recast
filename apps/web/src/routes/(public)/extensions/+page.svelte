<script lang="ts">
	import {
	  Container,
	  Footer,
	  HeroBackdrop,
	  Reveal,
	  Section,
	  SectionHeader,
	  SeoMeta,
	} from "$lib/components";
	import { prefersReducedMotion } from "$lib/motion-core";
	import {
	  ArrowRight,
	  Blocks,
	  Check,
	  Download,
	  Image as ImageIcon
	} from "@lucide/svelte";
	import { GithubBrand } from "@recast/ui/brand-icons";
	import { Button } from "@recast/ui/button";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";
	import { kinds, steps, trust } from "./data";
	
	// Hero entrance: same 80ms stagger as the rest of the public pages.
	// 460ms per element lands the whole ladder in well under a second.
	const reduced = $derived(prefersReducedMotion());
	const heroStagger = 80;
	const riseM = (delay: number) =>
		reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };


</script>

<SeoMeta
	title="Extensions for Recast"
	description="Install community asset packs right inside Recast: new cursors, backgrounds, gradients and motion presets. They carry no code, get checked by hash on install, and ask for nothing. Free, offline and open."
	eyebrow="Extensions"
/>

<main class="text-foreground">
	<!-- Hero -->
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-20">
		<HeroBackdrop src="/background-extensions.webp" tone="strong" />
		<Container class="relative">
			<div class="relative z-10 mx-auto flex max-w-3xl flex-col items-center gap-7 text-center">
				<span
					in:fly={riseM(heroStagger * 0)}
					class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70"
				>
					<span class="size-1.5 rounded-full bg-primary"></span>
					Extensions
				</span>
				<h1
					in:fly={riseM(heroStagger * 1)}
					class="text-balance text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5rem]"
				>
					Make Recast yours.
					<span class="block font-medium italic text-foreground/40">Open packs, no lock-in.</span>
				</h1>
				<p
					in:fly={riseM(heroStagger * 2)}
					class="text-pretty max-w-2xl text-base leading-relaxed text-muted-foreground sm:text-lg"
				>
					Community packs install straight into the editor's pickers. Each one is a manifest and a few static files, hash-checked. Nothing runs, nothing asks for permission.
				</p>
				<Reveal variant="up" delay={120} class="mt-2 flex flex-wrap items-center justify-center gap-3">
					<Button href="/download" variant="dark" class="gap-2">
						<Download class="size-4" />
						Get the app
					</Button>
					<Button
						href="https://github.com/kanakkholwal/recast/tree/main/extensions"
						variant="dark"
						class="gap-2"
					>
						<GithubBrand class="size-4" />
						Browse the registry
					</Button>
				</Reveal>
			</div>
		</Container>
	</Section>

	<!-- What a pack can add -->
	<Section id="kinds" class="border-t border-border-low/60 bg-foreground/1.5 dark:bg-foreground/2">
		<Container>
			<SectionHeader
				eyebrow="What a pack adds"
				title="It shows up where you already work."
				description="A pack feeds the pickers you already use. Nothing new to learn."
			/>
			<div class="mt-16 grid grid-cols-1 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-2 lg:grid-cols-3">
				{#each kinds as kind, i}
					{@const Icon = kind.icon}
					<Reveal variant="morph" delay={i * 70} class="h-full">
						<div class="flex h-full flex-col gap-3 bg-background/50 p-6 backdrop-blur-md">
							<Icon class="size-5 text-primary" />
							<div>
								<div class="text-sm font-semibold text-foreground">{kind.title}</div>
								<div class="mt-1.5 text-xs leading-relaxed text-muted-foreground">{kind.description}</div>
							</div>
						</div>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- How it works -->
	<Section id="how" class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="How it works"
				title="Three clicks, not a config file."
				align="center"
			/>
			<div class="mt-16 grid grid-cols-1 gap-8 md:grid-cols-3">
				{#each steps as step, i}
					{@const Icon = step.icon}
					<Reveal variant="up" delay={i * 80} class="h-full">
						<div class="flex h-full flex-col gap-4 rounded-2xl border border-border-low/40 bg-background/50 p-7">
							<div class="flex items-center gap-3">
								<span class="glass-chip grid size-9 shrink-0 place-items-center rounded-lg text-primary">
									<Icon class="size-4" />
								</span>
								<span class="text-[11px] font-bold uppercase tracking-[0.16em] text-muted-foreground/70">
									Step {i + 1}
								</span>
							</div>
							<div>
								<div class="text-base font-semibold text-foreground">{step.title}</div>
								<div class="mt-1.5 text-sm leading-relaxed text-muted-foreground">{step.description}</div>
							</div>
						</div>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Safe by design -->
	<Section id="safe" class="border-t border-border-low/60 bg-foreground/1.5 dark:bg-foreground/2">
		<Container>
			<SectionHeader
				eyebrow="Safe by design"
				title="Installable, without the install-anything risk."
				description="No code runs. A pack is just assets, so plugin supply-chain attacks don't apply."
			/>
			<div class="mt-16 grid grid-cols-1 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-2 lg:grid-cols-4">
				{#each trust as t, i}
					{@const Icon = t.icon}
					<Reveal variant="blur" delay={i * 70} class="h-full">
						<div class="flex h-full flex-col gap-3 bg-background/50 p-6 backdrop-blur-md">
							<Icon class="size-5 text-primary" />
							<div>
								<div class="text-sm font-semibold text-foreground">{t.title}</div>
								<div class="mt-1.5 text-xs leading-relaxed text-muted-foreground">{t.description}</div>
							</div>
						</div>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Build & share -->
	<Section id="build" class="border-t border-border-low/60">
		<Container>
			<div class="grid items-center gap-14 lg:grid-cols-12 lg:gap-20">
				<div class="lg:col-span-6">
					<SectionHeader
						eyebrow="Build & share"
						title="Make a pack in an afternoon."
						description="Drop your SVGs or images in a folder, write a manifest, open a PR. CI checks the rest."
					/>
					<ul class="mt-10 space-y-3.5">
						{#each [
							{ icon: ImageIcon, title: "Add your assets", description: "Cursors as SVG, backgrounds as PNG, JPG or WebP. Bare files, sitting in your pack's assets folder." },
							{ icon: Blocks, title: "Write the manifest", description: "A few lines of JSON that say what each file contributes. No URLs or hashes to write by hand." },
							{ icon: Check, title: "Verify and open a PR", description: "Run pnpm verify:extensions, push, and the Verify Extensions workflow takes it from there." },
						] as f, i}
							{@const Icon = f.icon}
							<Reveal as="li" variant="left" delay={i * 70} class="flex items-start gap-3.5">
								<span class="glass-chip mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg text-primary">
									<Icon class="size-4" />
								</span>
								<span>
									<span class="text-sm font-semibold text-foreground">{f.title}</span>
									<span class="block text-sm leading-relaxed text-muted-foreground">{f.description}</span>
								</span>
							</Reveal>
						{/each}
					</ul>
					<div class="mt-9 flex flex-wrap gap-3">
						<Button
							href="https://github.com/kanakkholwal/recast/tree/main/extensions"
							class="gap-2"
							variant="dark"
						>
							<GithubBrand class="size-4" />
							Read the contributor guide
						</Button>
						<Button href="/download" variant="ghost" class="gap-2">
							Get the app
							<ArrowRight class="size-4" />
						</Button>
					</div>
				</div>

				<div class="lg:col-span-6">
					<Reveal variant="morph">
						<div class="glass-card relative overflow-hidden rounded-2xl p-1.5 shadow-craft-lg">
							<div class="flex h-9 items-center gap-2 rounded-t-xl border-b border-border-low/40 bg-white/5 px-4">
								<Blocks class="size-3.5 text-primary" />
								<span class="text-[11px] font-medium text-muted-foreground">extension.json</span>
							</div>
							<pre class="overflow-x-auto rounded-b-xl bg-background/60 p-5 text-[11.5px] leading-relaxed text-muted-foreground"><code>{`{
  "id": "my-cursors",
  "name": "My Cursors",
  "version": "1.0.0",
  "kind": "asset-pack",
  "permissions": [],
  "contributes": {
    "cursors": [
      { "id": "ring", "label": "Ring",
        "rest": "ring",
        "hotspot": { "x": 32, "y": 32 } }
    ]
  },
  "assets": [
    { "id": "ring", "file": "assets/ring.svg" }
  ]
}`}</code></pre>
						</div>
					</Reveal>
				</div>
			</div>
		</Container>
	</Section>

	<Footer />
</main>
