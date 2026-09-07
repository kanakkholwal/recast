<script lang="ts">
import { ArrowRight, Blocks, Check, Compass, Download, Image as ImageIcon } from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import { Container, Footer, Reveal, Section, SectionLabel, SeoMeta } from "$lib/components";
import { kinds, steps, trust } from "./data";

const REGISTRY_URL = "https://github.com/kanakkholwal/recast/tree/main/extensions";

const heroFacts = [
	{ icon: Download, label: "Free and offline" },
	{ icon: Check, label: "Open registry" },
	{ icon: Blocks, label: "Assets, never code" },
];

const authoring = [
	{
		icon: ImageIcon,
		title: "Add your assets",
		description: "Cursors as SVG, backgrounds as PNG, JPG or WebP. Bare files in the pack folder.",
	},
	{
		icon: Blocks,
		title: "Write the manifest",
		description:
			"A few lines of JSON saying what each file contributes. No hashes to write by hand.",
	},
	{
		icon: Check,
		title: "Verify and open a PR",
		description: "Run pnpm verify:extensions, push, and the Verify Extensions workflow takes over.",
	},
];

const manifest = `{
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
}`;
</script>

<SeoMeta
	title="Extensions for Recast"
	description="Install community asset packs right inside Recast: new cursors, backgrounds, gradients and motion presets. They carry no code, get checked by hash on install, and ask for nothing. Free, offline and open."
	eyebrow="Extensions"
/>

<main class="text-foreground">
	<section class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40">
		<Container class="pb-12">
			<Reveal variant="up">
				<SectionLabel icon={Compass} label="Extensions" accent="lavender" />
			</Reveal>
			<Reveal variant="up" delay={60} class="mt-5">
				<h1 class="max-w-2xl font-display text-balance text-heading-lg md:text-display">
					Make Recast yours
				</h1>
			</Reveal>
			<Reveal variant="up" delay={120} class="mt-4">
				<p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
					Community packs install into the editor's own pickers. Each one is a manifest and a few
					static files, hash-checked.
				</p>
			</Reveal>
			<Reveal variant="up" delay={180} class="mt-8 flex flex-wrap items-center gap-3">
				<Button href="/download" variant="dark" class="gap-2">
					<Download class="size-4" />
					Get the app
				</Button>
				<Button href={REGISTRY_URL} variant="outline" class="gap-2" target="_blank">
					<GithubBrand class="size-4" />
					Browse the registry
				</Button>
			</Reveal>
		</Container>

		<Container class="border-t border-border-low">
			<ul class="flex flex-wrap items-center divide-x divide-border-low py-4">
				{#each heroFacts as fact (fact.label)}
					{@const Icon = fact.icon}
					<li
						class="inline-flex items-center gap-2 pr-4 text-body-sm text-muted-foreground not-first:pl-4"
					>
						<Icon class="size-4 shrink-0" />
						{fact.label}
					</li>
				{/each}
			</ul>
		</Container>
	</section>

	<Section id="kinds" class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Blocks} label="What a pack adds" accent="lavender" />
				</div>
			</Reveal>

			<div class="max-w-lg py-10">
				<Reveal variant="up" delay={60}>
					<h2 class="font-display text-balance text-heading md:text-heading-lg">
						It shows up where you already work
					</h2>
				</Reveal>
				<Reveal variant="up" delay={120} class="mt-4">
					<p class="text-pretty text-body-lg text-muted-foreground">
						A pack feeds the pickers you already use. Nothing new to learn.
					</p>
				</Reveal>
			</div>

			<div class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low sm:grid-cols-2 lg:grid-cols-3">
				{#each kinds as kind, i (kind.title)}
					{@const Icon = kind.icon}
					<Reveal
						variant="up"
						delay={i * 60}
						as="article"
						class="flex h-full flex-col bg-background px-6 py-8"
					>
						<Icon class="size-5 text-tag-lavender [fill-opacity:0.2]" fill="currentColor" />
						<h3 class="mt-4 font-display text-body font-medium text-foreground">{kind.title}</h3>
						<p class="mt-2 text-body-sm text-muted-foreground">{kind.description}</p>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<Section id="how" class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Download} label="How it works" accent="lavender" />
					<Button href="/download" variant="outline" size="sm" class="ml-auto shrink-0">
						Download free
					</Button>
				</div>
			</Reveal>

			<div class="max-w-lg py-10">
				<Reveal variant="up" delay={60}>
					<h2 class="font-display text-balance text-heading md:text-heading-lg">
						Three clicks, not a config file
					</h2>
				</Reveal>
			</div>

			<!-- Numbered chapters, same numeral treatment the home page uses. -->
			<div class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low md:grid-cols-3">
				{#each steps as step, i (step.title)}
					<Reveal
						variant="up"
						delay={i * 80}
						as="article"
						class="flex h-full flex-col bg-background px-6 py-8"
					>
						<span class="font-display text-heading-sm leading-none tabular-nums text-border-strong">
							{String(i + 1).padStart(2, "0")}
						</span>
						<h3 class="mt-5 font-display text-body font-medium text-foreground">{step.title}</h3>
						<p class="mt-2 text-body-sm text-muted-foreground">{step.description}</p>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<Section id="safe" class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Check} label="Safe by design" accent="green" />
				</div>
			</Reveal>

			<div class="grid gap-10 py-10 md:grid-cols-12 md:gap-12">
				<div class="md:col-span-5">
					<Reveal variant="up" delay={60}>
						<h2 class="font-display text-balance text-heading md:text-heading-lg">
							Installable, without the install-anything risk
						</h2>
					</Reveal>
					<Reveal variant="up" delay={120} class="mt-4">
						<p class="text-pretty text-body-lg text-muted-foreground">
							No code runs. A pack is assets, so plugin supply-chain attacks do not apply.
						</p>
					</Reveal>
				</div>

				<ul class="divide-y divide-border-low md:col-span-6 md:col-start-7">
					{#each trust as item, i (item.title)}
						{@const Icon = item.icon}
						<Reveal variant="up" delay={160 + i * 70} as="li" class="flex gap-4 py-5">
							<Icon
								class="mt-0.5 size-5 shrink-0 text-tag-green [fill-opacity:0.2]"
								fill="currentColor"
							/>
							<div class="min-w-0">
								<h3 class="font-display text-body font-medium text-foreground">{item.title}</h3>
								<p class="mt-1 text-body-sm text-muted-foreground">{item.description}</p>
							</div>
						</Reveal>
					{/each}
				</ul>
			</div>
		</Container>
	</Section>

	<Section id="build" class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Blocks} label="Build and share" accent="lavender" />
				</div>
			</Reveal>

			<div class="grid gap-10 py-10 md:grid-cols-12 md:gap-12">
				<div class="md:col-span-5">
					<Reveal variant="up" delay={60}>
						<h2 class="font-display text-balance text-heading md:text-heading-lg">
							Make a pack in an afternoon
						</h2>
					</Reveal>
					<Reveal variant="up" delay={120} class="mt-4">
						<p class="text-pretty text-body-lg text-muted-foreground">
							Drop your files in a folder, write a manifest, open a PR. CI checks the rest.
						</p>
					</Reveal>

					<ul class="mt-8 divide-y divide-border-low border-y border-border-low">
						{#each authoring as item, i (item.title)}
							{@const Icon = item.icon}
							<Reveal variant="up" delay={160 + i * 70} as="li" class="flex gap-4 py-5">
								<Icon class="mt-0.5 size-5 shrink-0 text-muted-foreground" />
								<div class="min-w-0">
									<h3 class="font-display text-body font-medium text-foreground">{item.title}</h3>
									<p class="mt-1 text-body-sm text-muted-foreground">{item.description}</p>
								</div>
							</Reveal>
						{/each}
					</ul>

					<Reveal variant="up" delay={380} class="mt-8 flex flex-wrap gap-3">
						<Button href={REGISTRY_URL} variant="dark" class="gap-2" target="_blank">
							<GithubBrand class="size-4" />
							Contributor guide
						</Button>
						<Button href="/download" variant="outline" class="group/cta gap-2">
							Get the app
							<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
						</Button>
					</Reveal>
				</div>

				<div class="md:col-span-7">
					<Reveal variant="up" delay={200}>
						<figure class="surface overflow-hidden">
							<figcaption
								class="flex items-center gap-2 border-b border-border-low bg-paper px-4 py-2.5 text-caption font-medium text-muted-foreground"
							>
								<Blocks class="size-3.5" />
								extension.json
							</figcaption>
							<pre
								class="overflow-x-auto px-5 py-4 font-mono text-caption leading-relaxed text-foreground"><code
									>{manifest}</code
								></pre>
						</figure>
					</Reveal>
				</div>
			</div>
		</Container>
	</Section>

	<Footer />
</main>
