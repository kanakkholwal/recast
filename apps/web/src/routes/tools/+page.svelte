<script lang="ts">
import {
	ArrowRight,
	Download,
	Image,
	MessageSquare,
	MousePointerClick,
	ShieldCheck,
	Upload,
	UserX,
	WifiOff,
	Wrench,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { LocalIcon } from "@recast/ui/local-icon";
import { Container, Footer, Reveal, Section, SectionLabel, SeoMeta } from "$lib/components";
import { TOOLS } from "$lib/tools/registry";
import { toolIcon } from "$lib/tools/tool-icons";

const REQUEST_URL = "https://github.com/kanakkholwal/recast/issues/new";

const heroFacts = [
	{ icon: WifiOff, label: "Runs in your browser" },
	{ icon: ShieldCheck, label: "Nothing is uploaded" },
	{ icon: UserX, label: "No account" },
];

const steps = [
	{
		icon: MousePointerClick,
		title: "Pick a tool",
		body: "Convert, trim, compress, or mute. Each one is a single focused page.",
	},
	{
		icon: Upload,
		title: "Drop your file",
		body: "It loads into your browser's own video engine.",
	},
	{
		icon: Download,
		title: "Save the result",
		body: "Download the output instantly. No watermark, no wait.",
	},
];
</script>

<SeoMeta
	title="Free Browser Video Tools"
	description="Convert, trim, compress, and extract from video for free. Everything runs in your browser. Your files are never uploaded."
	eyebrow="Tools"
/>

<main class="text-foreground">
	<section class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40">
		<Container class="pb-12">
			<Reveal variant="up">
				<SectionLabel icon={Wrench} label="Tools" accent="green" />
			</Reveal>
			<Reveal variant="up" delay={60} class="mt-5">
				<h1 class="max-w-2xl font-display font-semibold text-balance text-heading-lg md:text-display">
					Free video tools
				</h1>
			</Reveal>
			<Reveal variant="up" delay={120} class="mt-4">
				<p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
					Quick conversions that run entirely on your device. Pick one, drop a file, save the
					result.
				</p>
			</Reveal>
			<Reveal variant="up" delay={180} class="mt-8 flex flex-wrap items-center gap-3">
				<Button href="#tools" variant="dark" class="group/cta gap-2">
					Browse {TOOLS.length} tools
					<ArrowRight
						class="size-4 transition-transform group-hover/cta:translate-x-0.5 motion-reduce:transition-none"
					/>
				</Button>
				<Button href="/tools/screenshot-editor" variant="outline" class="gap-2">
					<Image class="size-4" />
					Screenshot editor
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

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<a
					href="/tools/screenshot-editor"
					class="group/spot flex flex-col gap-6 border-y border-border-low py-8 transition-colors hover:bg-paper motion-reduce:transition-none sm:flex-row sm:items-center sm:px-6"
				>
					<Image
						class="size-6 shrink-0 text-tag-green [fill-opacity:0.2]"
						fill="currentColor"
					/>
					<div class="min-w-0 flex-1">
						<h2 class="font-display text-heading-sm">Screenshot editor</h2>
						<p class="mt-2 max-w-xl text-pretty text-body-sm text-muted-foreground">
							Give a plain screenshot a gradient backdrop, a browser frame, a shadow and a 3D tilt,
							then export at up to 4x.
						</p>
					</div>
					<span
						class="inline-flex shrink-0 items-center gap-1.5 text-body-sm font-medium text-foreground"
					>
						Open editor
						<ArrowRight
							class="size-3.5 transition-transform group-hover/spot:translate-x-0.5 motion-reduce:transition-none"
						/>
					</span>
				</a>
			</Reveal>
		</Container>
	</Section>

	<Section id="tools" class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Wrench} label="Every tool" accent="green" />
					<span class="ml-auto shrink-0 text-body-sm tabular-nums text-muted-foreground">
						{TOOLS.length} tools
					</span>
				</div>
			</Reveal>

			<div class="max-w-lg py-10">
				<Reveal variant="up" delay={60}>
					<h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
						One page, one job
					</h2>
				</Reveal>
				<Reveal variant="up" delay={120} class="mt-4">
					<p class="text-pretty text-body-lg text-muted-foreground">
						No mode switcher, no settings tour. Each tool does the one conversion its name
						promises.
					</p>
				</Reveal>
			</div>

			<div
				class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low sm:grid-cols-2 lg:grid-cols-3"
			>
				{#each TOOLS as tool, i (tool.slug)}
					<Reveal
						variant="up"
						delay={Math.min(i, 6) * 50}
						as="article"
						class="flex h-full flex-col bg-background"
					>
						<a
							href={`/tools/${tool.slug}`}
							class="group/tool flex h-full flex-col px-6 py-8 transition-colors hover:bg-paper motion-reduce:transition-none"
						>
							<div class="flex items-start justify-between gap-3">
								<LocalIcon iconNode={toolIcon(tool.slug)} class="size-5 text-muted-foreground" />
								<span class="shrink-0 text-caption text-muted-foreground">{tool.outputLabel}</span>
							</div>
							<h3 class="mt-4 font-display text-body font-medium text-foreground">{tool.title}</h3>
							<p class="mt-2 text-body-sm text-muted-foreground">{tool.tagline}</p>
							<span
								class="mt-auto inline-flex items-center gap-1.5 pt-6 text-body-sm font-medium text-foreground"
							>
								Open
								<ArrowRight
									class="size-3.5 transition-transform group-hover/tool:translate-x-0.5 motion-reduce:transition-none"
								/>
							</span>
						</a>
					</Reveal>
				{/each}

				<Reveal
					variant="up"
					delay={350}
					as="article"
					class="flex h-full flex-col bg-background"
				>
					<a
						href={REQUEST_URL}
						target="_blank"
						rel="noopener noreferrer"
						class="group/req flex h-full flex-col px-6 py-8 transition-colors hover:bg-paper motion-reduce:transition-none"
					>
						<MessageSquare class="size-5 text-muted-foreground" />
						<h3 class="mt-4 font-display text-body font-medium text-foreground">
							Missing something?
						</h3>
						<p class="mt-2 text-body-sm text-muted-foreground">
							Tell us which conversion you need and we'll look at adding it.
						</p>
						<span
							class="mt-auto inline-flex items-center gap-1.5 pt-6 text-body-sm font-medium text-foreground"
						>
							Request a tool
							<ArrowRight
								class="size-3.5 transition-transform group-hover/req:translate-x-0.5 motion-reduce:transition-none"
							/>
						</span>
					</a>
				</Reveal>
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
					<h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
						Three steps, zero uploads
					</h2>
				</Reveal>
			</div>

			<div
				class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low md:grid-cols-3"
			>
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

	<Section class="mx-auto max-w-6xl" spacing="tight">
		<Container>
			<div class="max-w-xl py-6">
				<Reveal variant="up">
					<h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
						Want the full editor?
					</h2>
				</Reveal>
				<Reveal variant="up" delay={60} class="mt-4">
					<p class="text-pretty text-body-lg text-muted-foreground">
						These tools are the quick path. Recast for desktop records, polishes and exports a
						finished demo, offline and in one app.
					</p>
				</Reveal>
				<Reveal variant="up" delay={120} class="mt-8 flex flex-wrap items-center gap-3">
					<Button href="/download" variant="dark" class="gap-2">
						<Download class="size-4" />
						Download Recast
					</Button>
					<Button href="/features" variant="outline" class="group/cta gap-2">
						See what it does
						<ArrowRight
							class="size-4 transition-transform group-hover/cta:translate-x-0.5 motion-reduce:transition-none"
						/>
					</Button>
				</Reveal>
			</div>
		</Container>
	</Section>

	<Footer />
</main>
