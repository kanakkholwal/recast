<script lang="ts">
import {
	AppWindow,
	ArrowLeft,
	ArrowRight,
	Blend,
	Box,
	Clapperboard,
	Download,
	ImageIcon,
	Layers,
	MousePointerClick,
	Palette,
	ShieldCheck,
	Type,
	UserX,
	WifiOff,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Image } from "@unpic/svelte";
import {
	Container,
	FaqList,
	Footer,
	Reveal,
	Section,
	SectionLabel,
	SeoMeta,
} from "$lib/components";
import {
	buildEditorJsonLd,
	EDITOR_DESCRIPTION,
	EDITOR_FAQ,
	EDITOR_TITLE,
	UPSTREAM_URL,
} from "$lib/tools/screenshot-editor";

const jsonLd = buildEditorJsonLd();

// Stated once, on the rule under the hero. The old page also had a hero pill,
// a three-up privacy panel and a line in the CTA saying the same thing.
const heroFacts = [
	{ icon: WifiOff, label: "Runs offline" },
	{ icon: ShieldCheck, label: "Nothing is uploaded" },
	{ icon: UserX, label: "No account, no watermark" },
];

const features = [
	{
		icon: Palette,
		title: "Backdrops that do the work",
		body: "Gradients, mesh, patterns, solid colours, your own image, or transparent. Add blur and grain to taste.",
	},
	{
		icon: AppWindow,
		title: "Browser and device frames",
		body: "Drop the shot into a Safari or Chrome window, light or dark, with your own URL. Phone and tablet frames too.",
	},
	{
		icon: Box,
		title: "3D tilt and perspective",
		body: "Rotate and tilt the shot in real 3D space. Presets for a quick angle, sliders when you want it exact.",
	},
	{
		icon: Layers,
		title: "Glass and border styles",
		body: "Wrap the shot in a frosted glass card or a clean solid border, with shadows from subtle to dramatic.",
	},
	{
		icon: Type,
		title: "Text and annotations",
		body: "Add headings, arrows, boxes, and circles to point at the thing that matters.",
	},
	{
		icon: Blend,
		title: "Colour adjustments",
		body: "Brightness, contrast, saturation, hue, grayscale, and blur, applied to the shot and not the backdrop.",
	},
	{
		icon: Clapperboard,
		title: "Motion, then MP4",
		body: "Pick a motion preset, stretch the clip on the timeline, and export an MP4 for social.",
	},
	{
		icon: Download,
		title: "Export at up to 4x",
		body: "PNG or JPG at retina resolution, or copy straight to your clipboard.",
	},
];

const steps = [
	{
		icon: ImageIcon,
		title: "Drop your screenshot",
		body: "Upload it, paste it, or drag it onto the page. It loads straight into your browser.",
	},
	{
		icon: MousePointerClick,
		title: "Make it look good",
		body: "Pick a backdrop, round the corners, add a shadow, tilt it. A template gets you there in one click.",
	},
	{
		icon: Download,
		title: "Export or copy",
		body: "Save a PNG at up to 4x, or copy to the clipboard and paste it into your post.",
	},
];
</script>

<SeoMeta title={EDITOR_TITLE} description={EDITOR_DESCRIPTION} eyebrow="Tools" />

<svelte:head>
	{@html `<script type="application/ld+json">${jsonLd}</script>`}
</svelte:head>

<main class="text-foreground">
	<section class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40">
		<Container class="pb-12">
			<Reveal variant="up">
				<a
					href="/tools"
					class="group/back inline-flex items-center gap-1.5 text-body-sm font-medium text-muted-foreground transition-colors hover:text-foreground motion-reduce:transition-none"
				>
					<ArrowLeft
						class="size-3.5 transition-transform group-hover/back:-translate-x-0.5 motion-reduce:transition-none"
					/>
					All tools
				</a>
			</Reveal>
			<Reveal variant="up" delay={40} class="mt-6">
				<SectionLabel icon={ImageIcon} label="Screenshot editor" accent="green" />
			</Reveal>
			<Reveal variant="up" delay={100} class="mt-5">
				<h1 class="max-w-2xl font-display text-balance text-heading-lg md:text-display">
					Free screenshot editor
				</h1>
			</Reveal>
			<Reveal variant="up" delay={160} class="mt-4">
				<p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
					A raw screenshot looks like a bug report. Give it a backdrop, a shadow and a tilt, and it
					looks like a product.
				</p>
			</Reveal>
			<Reveal variant="up" delay={220} class="mt-8 flex flex-wrap items-center gap-3">
				<Button href="/tools/screenshot-editor/edit" variant="dark" class="group/cta gap-2">
					Open the editor
					<ArrowRight
						class="size-4 transition-transform group-hover/cta:translate-x-0.5 motion-reduce:transition-none"
					/>
				</Button>
				<Button href="/download" variant="outline" class="gap-2">
					<Download class="size-4" />
					Get the desktop app
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

	<!-- Before / after. The "before" is a CSS rendering of a bare screenshot, so
	     there is no asset to load for the half nobody looks at twice. -->
	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div
					class="grid items-center gap-px border-y border-border-low bg-border-low sm:grid-cols-2"
				>
					<figure class="flex flex-col gap-4 bg-background px-6 py-8">
						<div class="grid flex-1 place-items-center rounded-lg bg-paper p-8">
							<div class="shot" aria-hidden="true">
								<span class="bar w-3/5"></span>
								<span class="bar w-4/5"></span>
								<span class="bar w-2/5"></span>
							</div>
						</div>
						<figcaption class="text-caption text-muted-foreground">Your screenshot</figcaption>
					</figure>

					<figure class="flex flex-col gap-4 bg-background px-6 py-8">
						<div class="flex-1 overflow-hidden rounded-lg">
							<Image
								src="/screenshots/reshot_preview.webp"
								alt="The same screenshot with a gradient backdrop, browser frame, shadow and 3D tilt"
								height="400"
								width="600"
								class="h-full w-full object-cover"
							/>
						</div>
						<figcaption class="text-caption font-medium text-foreground">
							Thirty seconds later
						</figcaption>
					</figure>
				</div>
			</Reveal>
		</Container>
	</Section>

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Palette} label="What you get" accent="green" />
					<Button
						href="/tools/screenshot-editor/edit"
						variant="outline"
						size="sm"
						class="ml-auto shrink-0"
					>
						Open the editor
					</Button>
				</div>
			</Reveal>

			<div class="max-w-lg py-10">
				<Reveal variant="up" delay={60}>
					<h2 class="font-display text-balance text-heading md:text-heading-lg">
						Everything a screenshot needs to look intentional
					</h2>
				</Reveal>
			</div>

			<div
				class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low sm:grid-cols-2 lg:grid-cols-4"
			>
				{#each features as feature, i (feature.title)}
					{@const Icon = feature.icon}
					<Reveal
						variant="up"
						delay={Math.min(i, 4) * 60}
						as="article"
						class="flex h-full flex-col bg-background px-6 py-8"
					>
						<Icon class="size-5 text-tag-green [fill-opacity:0.2]" fill="currentColor" />
						<h3 class="mt-4 font-display text-body font-medium text-foreground">{feature.title}</h3>
						<p class="mt-2 text-body-sm text-muted-foreground">{feature.body}</p>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={MousePointerClick} label="How it works" accent="green" />
				</div>
			</Reveal>

			<div class="max-w-lg py-10">
				<Reveal variant="up" delay={60}>
					<h2 class="font-display text-balance text-heading md:text-heading-lg">
						Three steps, zero uploads
					</h2>
				</Reveal>
			</div>

			<div class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low md:grid-cols-3">
				{#each steps as step, i (step.title)}
					{@const Icon = step.icon}
					<Reveal
						variant="up"
						delay={i * 80}
						as="article"
						class="flex h-full flex-col bg-background px-6 py-8"
					>
						<span class="font-display text-heading-sm leading-none tabular-nums text-border-strong">
							{String(i + 1).padStart(2, "0")}
						</span>
						<Icon class="mt-5 size-5 text-muted-foreground" />
						<h3 class="mt-4 font-display text-body font-medium text-foreground">{step.title}</h3>
						<p class="mt-2 text-body-sm text-muted-foreground">{step.body}</p>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<div class="max-w-2xl">
				<h2 class="mb-6 font-display text-balance text-heading md:text-heading-lg">Questions</h2>
				<FaqList items={EDITOR_FAQ} />
			</div>
		</Container>
	</Section>

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<div class="max-w-xl py-6">
				<Reveal variant="up">
					<h2 class="font-display text-balance text-heading md:text-heading-lg">
						Make your next screenshot look intentional
					</h2>
				</Reveal>
				<Reveal variant="up" delay={60} class="mt-4">
					<p class="text-pretty text-body-lg text-muted-foreground">
						Open it and drop an image in. Nothing to install, nothing to sign up for.
					</p>
				</Reveal>
				<Reveal variant="up" delay={120} class="mt-8 flex flex-wrap items-center gap-3">
					<Button href="/tools/screenshot-editor/edit" variant="dark" class="group/cta gap-2">
						Open the editor
						<ArrowRight
							class="size-4 transition-transform group-hover/cta:translate-x-0.5 motion-reduce:transition-none"
						/>
					</Button>
					<Button href="/tools" variant="outline">Browse all tools</Button>
				</Reveal>
			</div>
		</Container>
	</Section>

	<!-- Credit. Their work shaped this, so say so where people can see it. -->
	<Section class="mx-auto max-w-6xl sr-only" spacing="tight">
		<Container>
			<p class="max-w-2xl text-body-sm text-muted-foreground">
				This editor is a Svelte port of
				<a
					href={UPSTREAM_URL}
					target="_blank"
					rel="noopener noreferrer"
					class="font-medium text-foreground underline-offset-4 hover:underline"
				>
					Screenshot Studio
				</a>
				by Kartik Labhshetwar, used under the Apache 2.0 license. It is a great tool and it shaped what
				this one does.
			</p>
		</Container>
	</Section>

	<Footer />
</main>

<style>
	/* Illustration only: a literal rendering of a bare, unstyled screenshot, so
	   the colours are fixed by design and do not follow the app theme. */
	.shot {
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 0.55rem;
		width: 100%;
		max-width: 20rem;
		aspect-ratio: 16 / 10;
		padding: 1.1rem;
		background: #ffffff;
		border: 1px solid #dcdce1;
		border-radius: 2px;
	}

	.shot .bar {
		height: 0.5rem;
		border-radius: 9999px;
		background: #e4e4e9;
	}
</style>
