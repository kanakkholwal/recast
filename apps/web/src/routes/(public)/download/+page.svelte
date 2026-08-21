<script lang="ts">
import { Container, Footer, Reveal, Section, SectionLabel, SeoMeta } from "$lib/components";
import {
  ArrowDownToLine,
  Check,
  ChevronDown,
  Download,
  LifeBuoy,
  ShieldCheck,
  TriangleAlert,
} from "@recast/icons";
import { AppleBrand, LinuxBrand, WindowsBrand } from "@recast/ui/brand-icons";
import { Button, buttonVariants } from "@recast/ui/button";
import * as Collapsible from "@recast/ui/collapsible";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import * as Tabs from "@recast/ui/tabs";
import { cn } from "@recast/ui/utils";
import type { PageData } from "./$types";
import type { OS } from "./data";
import {
  ISSUES_URL,
  installSteps,
  platforms,
  ships,
  stabilityCopy,
  systemRequirements,
} from "./data";

let { data }: { data: PageData } = $props();

let detectedOS = $state<OS>("Unknown");

$effect(() => {
	const ua = window.navigator.userAgent;
	if (ua.includes("Mac")) detectedOS = "macOS";
	else if (ua.includes("Win")) detectedOS = "Windows";
	else if (ua.includes("Linux")) detectedOS = "Linux";
});

type Asset = { link: string | null; label: string };

const platformAssets = $derived<Record<Exclude<OS, "Unknown">, Asset[]>>({
	macOS: [
		{ link: data.downloads.macosAppleSilicon, label: "Apple Silicon (.dmg)" },
		{ link: data.downloads.macosIntel, label: "Intel (.dmg)" },
	],
	Windows: [
		{ link: data.downloads.windowsExe, label: "Installer (.exe)" },
		{ link: data.downloads.windowsMsi, label: "Installer (.msi)" },
	],
	Linux: [
		{ link: data.downloads.linuxAppImage, label: "AppImage (universal)" },
		{ link: data.downloads.linuxDeb, label: "Debian / Ubuntu (.deb)" },
		{ link: data.downloads.linuxRpm, label: "Red Hat / Fedora (.rpm)" },
	],
});

const primary = $derived(detectedOS !== "Unknown" ? platformAssets[detectedOS][0] : null);
const secondary = $derived(detectedOS !== "Unknown" ? platformAssets[detectedOS].slice(1) : []);

const activeTab = $derived(detectedOS !== "Unknown" ? detectedOS : "Windows");

const detectedIcon = $derived(
	detectedOS === "macOS"
		? AppleBrand
		: detectedOS === "Windows"
			? WindowsBrand
			: detectedOS === "Linux"
				? LinuxBrand
				: Download,
);
</script>

<SeoMeta
	title="Download Recast"
	description="Download Recast for macOS, Windows, or Linux. Free during beta. The native screen recorder for makers shipping every week."
	eyebrow="Download"
	pageTitle="Download Recast for macOS, Windows, and Linux"
/>

<main class="text-foreground">
	<section class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40">
		<Container class="pb-12">
			<Reveal variant="up">
				<SectionLabel icon={Download} label="Download" />
			</Reveal>
			<Reveal variant="up" delay={60} class="mt-5">
				<h1 class="max-w-2xl font-semibold font-display text-balance text-heading-lg md:text-display">
					Get Recast for {detectedOS !== "Unknown" ? detectedOS : "your desktop"}
				</h1>
			</Reveal>
			<Reveal variant="up" delay={120} class="mt-4">
				<p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
					Free during beta, no sign-up. The native recorder for makers who would rather ship than
					open a timeline.
				</p>
			</Reveal>

			<Reveal variant="up" delay={180} class="mt-8 flex flex-wrap items-center gap-3">
				{#if primary?.link}
					{@const OSIcon = detectedIcon}
					<Button href={primary.link} variant="dark" size="lg" class="gap-2.5">
						<OSIcon class="size-4" />
						Download for {detectedOS}
						<ArrowDownToLine class="size-4 opacity-70" />
					</Button>
					{#if secondary.length}
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								class={cn(buttonVariants({ variant: "outline", size: "lg" }), "group/menu gap-2")}
								aria-label="Other builds for {detectedOS}"
							>
								Other builds
								<ChevronDown
									class="size-4 transition-transform duration-200 group-data-[state=open]/menu:rotate-180"
								/>
							</DropdownMenu.Trigger>
							<DropdownMenu.Content align="start" sideOffset={8} class="w-64 p-1.5">
								{#each secondary as opt (opt.label)}
									<DropdownMenu.Item
										class="cursor-pointer rounded-md px-2.5 py-2 text-body-sm"
										onclick={() => opt.link && (window.location.href = opt.link)}
									>
										{opt.label}
									</DropdownMenu.Item>
								{/each}
								<DropdownMenu.Separator class="my-1.5" />
								<a
									href="#all-platforms"
									class="flex items-center justify-between gap-3 rounded-md px-2.5 py-2 text-body-sm text-muted-foreground transition-colors hover:text-foreground"
								>
									All platforms
									<ArrowDownToLine class="size-3.5" />
								</a>
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{/if}
				{:else}
					<Button href="#all-platforms" variant="dark" size="lg" class="gap-2">
						View all platforms
						<ArrowDownToLine class="size-4" />
					</Button>
				{/if}

				<span class="text-body-sm text-muted-foreground">
					Version {data.version}
				</span>
			</Reveal>

			{#if detectedOS === "macOS"}
				<Reveal variant="up" delay={220} class="mt-5">
					<a
						href="#macos-first-launch"
						class="inline-flex items-center gap-2 text-body-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
					>
						<TriangleAlert class="size-4 shrink-0 text-tag-tangerine" />
						macOS needs Homebrew, or one Terminal step to clear Gatekeeper
					</a>
				</Reveal>
			{/if}
		</Container>

		<!-- Build honesty, stated before anyone clicks. -->
		<Container class="border-t border-border-low">
			<div class="flex flex-wrap items-center gap-x-6 gap-y-3 py-4">
				<ul class="flex flex-wrap items-center divide-x divide-border-low">
					{#each platforms as p (p.id)}
						{@const s = stabilityCopy[p.stability]}
						<li
							class="inline-flex items-center gap-2 pr-4 text-body-sm text-muted-foreground not-first:pl-4"
						>
							<span class={cn("size-1.5 shrink-0 rounded-full", s.dot)}></span>
							{p.title}
							<span class="text-muted-foreground">
								{p.stability === "stable" ? "stable" : "early port"}
							</span>
						</li>
					{/each}
				</ul>
				<p class="text-body-sm text-muted-foreground">
					Windows is the daily-driver build. Hit something?
					<a
						href={ISSUES_URL}
						target="_blank"
						rel="noopener noreferrer"
						class="text-foreground underline-offset-4 hover:underline"
					>
						File an issue
					</a>
					and I reply personally.
				</p>
			</div>
		</Container>
	</section>

	<!-- Ships with every build -->
	<section class="mx-auto w-full max-w-6xl border-b border-border-low">
		<Container>
			<div class="grid grid-cols-2 gap-px bg-border-low lg:grid-cols-4">
				{#each ships as ship, i (ship.label)}
					{@const Icon = ship.icon}
					<Reveal variant="up" delay={i * 70} class="flex flex-col bg-background px-6 py-8">
						<Icon class="size-5 text-muted-foreground" />
						<h2 class="mt-4 font-display text-body font-medium text-foreground">{ship.label}</h2>
						<p class="mt-1 text-body-sm text-muted-foreground">{ship.value}</p>
					</Reveal>
				{/each}
			</div>
		</Container>
	</section>

	<Section id="system-requirements" class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={ShieldCheck} label="System requirements" />
				</div>
			</Reveal>

			<div class="grid gap-10 py-10 md:grid-cols-12 md:gap-12">
				<div class="md:col-span-4">
					<Reveal variant="up" delay={60}>
						<h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
							Records on the machine you have
						</h2>
					</Reveal>
					<Reveal variant="up" delay={120} class="mt-4">
						<p class="text-pretty text-body-sm text-muted-foreground">
							Recast probes NVIDIA, AMD and Intel at startup. If none initialise it falls back to
							libx264 on the CPU, tuned for low-latency capture. Hardware encoders only let your CPU
							breathe.
						</p>
					</Reveal>
				</div>

				<div class="md:col-span-8">
					<Reveal variant="up" delay={160}>
						<div
							class="grid grid-cols-[7rem_1fr_1fr] border-b border-border-low py-3 text-caption font-medium text-muted-foreground"
						>
							<span>Component</span>
							<span class="px-4">Minimum</span>
							<span class="px-4 text-foreground">Recommended</span>
						</div>
						<ul class="divide-y divide-border-low border-b border-border-low">
							{#each systemRequirements as req (req.label)}
								{@const Icon = req.icon}
								<li class="grid grid-cols-[7rem_1fr_1fr] items-start py-4">
									<span class="flex items-center gap-2 text-body-sm font-medium text-foreground">
										<Icon class="size-4 shrink-0 text-muted-foreground" />
										{req.label}
									</span>
									<span class="px-4 text-body-sm text-muted-foreground">{req.minimum}</span>
									<span class="px-4 text-body-sm text-foreground">{req.recommended}</span>
								</li>
							{/each}
						</ul>
					</Reveal>
				</div>
			</div>
		</Container>
	</Section>

	<Section id="all-platforms" class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Download} label="All platforms" accent="green" />
					<a
						href="https://github.com/kanakkholwal/recast/releases/latest"
						target="_blank"
						rel="noopener noreferrer"
						class="ml-auto shrink-0 text-body-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
					>
						Checksums
					</a>
				</div>
			</Reveal>

			<Reveal variant="up" delay={60} class="mt-10">
				<h2 class="max-w-lg font-display text-balance text-heading md:text-heading-lg">
					Pick your build
				</h2>
			</Reveal>

			<div class="mt-8">
				<Tabs.Root value={activeTab} class="w-full">
					<Tabs.List
						class="inline-flex h-auto gap-1 rounded-lg border border-border-low bg-paper p-1"
					>
						{#each platforms as p (p.id)}
							{@const Icon = p.icon}
							<Tabs.Trigger
								value={p.id}
								class="flex items-center gap-2 rounded-md px-3 py-1.5 text-body-sm font-medium text-muted-foreground data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-craft-sm"
							>
								<Icon class="size-4" />
								{p.title}
							</Tabs.Trigger>
						{/each}
					</Tabs.List>

					{#each platforms as p (p.id)}
						{@const Icon = p.icon}
						{@const guide = installSteps[p.id]}
						{@const anchorId =
							p.id === "macOS" ? "macos-first-launch" : `install-${p.id.toLowerCase()}`}
						{@const s = stabilityCopy[p.stability]}
						<Tabs.Content value={p.id} class="mt-8">
							<div class="grid gap-10 border-t border-border-low pt-8 md:grid-cols-12 md:gap-12">
								<div class="md:col-span-5">
									<div class="flex items-center gap-2.5">
										<Icon class="size-5 text-foreground" />
										<h3 class="font-display text-subheading font-medium text-foreground">
											{p.title}
										</h3>
										<span
											class="inline-flex items-center gap-1.5 text-caption font-medium text-muted-foreground"
										>
											<span class={cn("size-1.5 rounded-full", s.dot)}></span>
											{p.stability === "stable" ? "Stable" : "Beta"}
										</span>
									</div>
									<p class="mt-2 text-body-sm text-muted-foreground">{p.subtitle}</p>

									<div class="mt-6 flex flex-col gap-2">
										{#each platformAssets[p.id] as asset (asset.label)}
											<Button
												href={asset.link ?? undefined}
												disabled={!asset.link}
												variant={asset === platformAssets[p.id][0] ? "dark" : "outline"}
												class="w-full justify-between gap-3"
											>
												<span>{asset.label}</span>
												<ArrowDownToLine class="size-4 opacity-70" />
											</Button>
										{/each}
									</div>
								</div>

								<!-- Same shape for every platform, so adding an OS is a data change. -->
								<div id={anchorId} class="md:col-span-7">
									<p class="text-body-sm text-muted-foreground">{guide.intro}</p>

									<ol class="mt-6 divide-y divide-border-low border-y border-border-low">
										{#each guide.steps as step, idx (step.title)}
											<li class="flex gap-4 py-5">
												<span
													class="font-display text-body font-medium leading-6 tabular-nums text-border-strong"
												>
													{String(idx + 1).padStart(2, "0")}
												</span>
												<div class="min-w-0 flex-1">
													<h4 class="font-display text-body font-medium text-foreground">
														{step.title}
													</h4>
													<p class="mt-1 text-body-sm text-muted-foreground">{step.body}</p>
													{#if step.code}
														<pre
															class="mt-3 overflow-x-auto rounded-md border border-border-low bg-paper px-3 py-2.5 font-mono text-caption text-foreground"><code
																>{step.code}</code
															></pre>
													{/if}
													{#if step.hint}
														<p
															class="mt-2 flex items-start gap-1.5 text-caption text-muted-foreground"
														>
															<Check class="mt-0.5 size-3 shrink-0 text-tag-green" />
															<span>{step.hint}</span>
														</p>
													{/if}
												</div>
											</li>
										{/each}
									</ol>

									{#if guide.faqs.length}
										<div class="mt-8">
											<div class="flex items-center gap-2 text-body-sm font-medium text-foreground">
												<LifeBuoy class="size-4 text-tag-tangerine" />
												If something goes wrong
											</div>
											<div class="mt-3 divide-y divide-border-low border-y border-border-low">
												{#each guide.faqs as faq (faq.title)}
													<Collapsible.Root class="group/faq">
														<Collapsible.Trigger
															class="flex w-full cursor-pointer items-center justify-between gap-4 py-4 text-left text-body-sm font-medium text-foreground"
														>
															{faq.title}
															<ChevronDown
																class="size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-data-[state=open]/faq:rotate-180 motion-reduce:transition-none"
															/>
														</Collapsible.Trigger>
														<Collapsible.Content>
															<div class="pb-4">
																<p class="text-body-sm text-muted-foreground">{faq.body}</p>
																{#if faq.code}
																	<pre
																		class="mt-3 overflow-x-auto rounded-md border border-border-low bg-paper px-3 py-2.5 font-mono text-caption text-foreground"><code
																			>{faq.code}</code
																		></pre>
																{/if}
															</div>
														</Collapsible.Content>
													</Collapsible.Root>
												{/each}
											</div>
										</div>
									{/if}

									{#if p.id === "macOS"}
										<p class="mt-6 flex items-start gap-2.5 text-caption text-muted-foreground">
											<TriangleAlert class="mt-0.5 size-3.5 shrink-0 text-tag-tangerine" />
											<span>
												Until Apple notarization ships, the quarantine step above is required on
												the .dmg path. Homebrew clears it for you.
											</span>
										</p>
									{/if}
								</div>
							</div>
						</Tabs.Content>
					{/each}
				</Tabs.Root>
			</div>

			<Reveal variant="up" class="mt-12 border-t border-border-low pt-5">
				<p class="flex flex-wrap items-center gap-2 text-body-sm text-muted-foreground">
					<ShieldCheck class="size-4 shrink-0" />
					Every build is open source under GPLv3.
					<a
						href="https://github.com/kanakkholwal/recast"
						target="_blank"
						rel="noopener noreferrer"
						class="text-foreground underline-offset-4 hover:underline"
					>
						Read the source
					</a>
					or verify checksums on the
					<a
						href="https://github.com/kanakkholwal/recast/releases/latest"
						target="_blank"
						rel="noopener noreferrer"
						class="text-foreground underline-offset-4 hover:underline"
					>
						release page
					</a>
				</p>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
