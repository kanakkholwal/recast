<script lang="ts">
	import {
		Container,
		Eyebrow,
		Footer,
		Reveal,
		Section,
		SectionHeader,
	} from "$lib/components";
	import { Button } from "@recast/ui/button";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import * as Tabs from "@recast/ui/tabs";
	import { cn } from "@recast/ui/utils";
	import {
		Apple,
		ArrowDownToLine,
		ChevronDown,
		Download,
		FileBox,
		Monitor,
		ShieldCheck,
		Sparkles,
		Terminal,
		TriangleAlert,
		WifiOff,
		Zap,
	} from "lucide-svelte";
	import type { PageData } from "./$types";

	let { data }: { data: PageData } = $props();

	type OS = "macOS" | "Windows" | "Linux" | "Unknown";

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

	const primary = $derived(
		detectedOS !== "Unknown" ? platformAssets[detectedOS][0] : null,
	);
	const secondary = $derived(
		detectedOS !== "Unknown" ? platformAssets[detectedOS].slice(1) : [],
	);

	const platforms = [
		{
			id: "macOS" as const,
			icon: Apple,
			title: "macOS",
			subtitle: "Requires macOS 12.0 or later",
		},
		{
			id: "Windows" as const,
			icon: Monitor,
			title: "Windows",
			subtitle: "Requires Windows 10 or later",
		},
		{
			id: "Linux" as const,
			icon: Terminal,
			title: "Linux",
			subtitle: "Debian, Ubuntu, Fedora, Arch",
		},
	];

	let activeTab = $derived(detectedOS !== "Unknown" ? detectedOS : "macOS");

	const detectedIcon = $derived(
		detectedOS === "macOS"
			? Apple
			: detectedOS === "Windows"
				? Monitor
				: detectedOS === "Linux"
					? Terminal
					: Download,
	);

	const ships = [
		{ icon: WifiOff, label: "Offline-first", value: "Stays on disk" },
		{ icon: Zap, label: "GPU export", value: "Hardware-encoded" },
		{ icon: FileBox, label: "Open format", value: ".recast project" },
		{ icon: ShieldCheck, label: "Open source", value: "MIT licensed" },
	];
</script>

<svelte:head>
	<title>Download Recast — macOS, Windows, Linux</title>
	<meta
		name="description"
		content="Download Recast for macOS, Windows, or Linux. Free during beta. The native screen recorder for makers shipping every week."
	/>
</svelte:head>

<main class="text-foreground">
	<Section spacing="none" class="dl-atmosphere relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-24">
		<div aria-hidden="true" class="dl-aurora pointer-events-none absolute inset-0 -z-10"></div>
		<div aria-hidden="true" class="dl-grid pointer-events-none absolute inset-0 -z-10 opacity-[0.35]"></div>

		<Container class="relative">
			<div class="mx-auto flex max-w-3xl flex-col items-center text-center">
				<Eyebrow icon={Sparkles} variant="primary">
					Latest release · {data.version}
				</Eyebrow>

				<h1 class="text-balance mt-7 animate-fade-up text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl">
					Get Recast for
					<span class="mt-2 block font-medium italic text-foreground/40">
						{detectedOS !== "Unknown" ? detectedOS : "your desktop"}.
					</span>
				</h1>

				<p
					class="text-pretty mt-6 max-w-xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg"
					style="animation-delay: 120ms"
				>
					Free during beta. No sign-up. The native screen recorder for founders, indie hackers, and product engineers who'd rather ship than open a timeline.
				</p>

				<div
					class="mt-10 flex animate-fade-up flex-col items-center gap-3"
					style="animation-delay: 240ms"
				>
					{#if primary?.link}
						{@const OSIcon = detectedIcon}
						<div
							class="dl-cta group/dl flex items-stretch overflow-hidden rounded-2xl bg-foreground text-background shadow-craft-xl ring-1 ring-foreground/10 transition-all duration-200 hover:-translate-y-0.5 hover:shadow-craft-floating active:translate-y-0"
						>
							<a
								href={primary.link}
								class="flex items-center gap-3.5 px-5 py-3 transition-colors hover:bg-background/8 sm:gap-4 sm:px-6 sm:py-3.5"
							>
								<span class="grid size-10 place-items-center rounded-xl bg-background/10 ring-1 ring-background/15 sm:size-11">
									<OSIcon class="size-5" />
								</span>
								<span class="flex flex-col items-start leading-tight">
									<span class="text-sm font-semibold sm:text-base">
										Download for {detectedOS}
									</span>
									<span class="mt-0.5 font-mono text-[11px] font-medium opacity-60">
										{primary.label}
									</span>
								</span>
								<ArrowDownToLine class="ml-1 size-4 opacity-70 transition-transform group-hover/dl:translate-y-0.5 sm:ml-2" />
							</a>
							{#if secondary.length}
								<DropdownMenu.Root>
									<DropdownMenu.Trigger
										class="group/menu grid w-12 shrink-0 place-items-center border-l border-background/15 transition-colors hover:bg-background/8 sm:w-14"
										aria-label="Other architectures"
									>
										<ChevronDown
											class="size-4 opacity-80 transition-transform duration-200 ease-[cubic-bezier(0.625,0.05,0,1)] group-data-[state=open]/menu:rotate-180"
										/>
									</DropdownMenu.Trigger>
									<DropdownMenu.Content
										align="end"
										sideOffset={10}
										class="w-72 rounded-xl p-2 shadow-craft-lg"
									>
										<DropdownMenu.Label
											class="px-2.5 pt-1 pb-2 text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground"
										>
											Other builds for {detectedOS}
										</DropdownMenu.Label>
										{#each secondary as opt}
											{@const fmt = opt.label.match(/\(([^)]+)\)$/)?.[1] ?? ""}
											{@const name = opt.label.replace(/\s*\([^)]+\)$/, "")}
											<DropdownMenu.Item
												class="group/item flex cursor-pointer items-center justify-between gap-3 rounded-lg px-2 py-2 text-sm font-medium transition-colors duration-200 ease-[cubic-bezier(0.625,0.05,0,1)]"
												onclick={() => opt.link && (window.location.href = opt.link)}
											>
												<span class="flex items-center gap-2.5">
													<span class="grid size-8 place-items-center rounded-lg bg-foreground/5 ring-1 ring-foreground/5 transition-colors duration-200 group-hover/item:bg-primary/10 group-hover/item:ring-primary/20">
														<OSIcon class="size-4 opacity-70 transition-opacity group-hover/item:opacity-100" />
													</span>
													<span class="text-foreground/85">{name}</span>
												</span>
												<span class="font-mono text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground/80">
													{fmt}
												</span>
											</DropdownMenu.Item>
										{/each}
										<DropdownMenu.Separator class="my-1.5" />
										<a
											href="#all-platforms"
											class="flex items-center justify-between gap-3 rounded-lg px-2 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
										>
											<span>All platforms & checksums</span>
											<ArrowDownToLine class="size-3.5 opacity-60" />
										</a>
									</DropdownMenu.Content>
								</DropdownMenu.Root>
							{/if}
						</div>
					{:else}
						<Button href="#all-platforms" size="lg" class="gap-2">
							View all platforms
							<ArrowDownToLine class="size-4" />
						</Button>
					{/if}

					<a
						href="#all-platforms"
						class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground transition-colors hover:text-foreground"
					>
						Not on {detectedOS !== "Unknown" ? detectedOS : "this OS"}? See all platforms ↓
					</a>

					<!-- macOS users get a one-time Gatekeeper workaround. Surface it
					     up here so they see it BEFORE downloading and don't bounce
					     off the "is damaged" error. Anchors to the full instructions
					     in the macOS tab below. -->
					{#if detectedOS === "macOS"}
						<a
							href="#macos-first-launch"
							class="mt-1 inline-flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-[0.16em] text-amber-600 transition-colors hover:text-amber-500 dark:text-amber-400"
						>
							<TriangleAlert class="size-3" />
							macOS first launch needs a one-time Terminal step
						</a>
					{/if}
				</div>
			</div>

			<!-- Ships with every build -->
			<Reveal>
				<div class="mx-auto mt-20 grid max-w-4xl grid-cols-2 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-4">
					{#each ships as ship}
						{@const Icon = ship.icon}
						<div class="flex flex-col gap-2 bg-background/60 p-5 backdrop-blur-md">
							<Icon class="size-4 text-primary" />
							<div>
								<div class="text-sm font-semibold text-foreground">{ship.label}</div>
								<div class="mt-0.5 text-xs text-muted-foreground">{ship.value}</div>
							</div>
						</div>
					{/each}
				</div>
			</Reveal>
		</Container>
	</Section>

	<Section id="all-platforms" class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="All platforms"
				title="Pick your build."
				description="Native binaries for every supported platform and architecture."
			/>

			<div class="mt-12">
				<Tabs.Root value={activeTab} class="w-full">
					<Tabs.List class="glass-card grid w-full grid-cols-3 rounded-xl p-1 sm:max-w-md">
						{#each platforms as p}
							{@const Icon = p.icon}
							<Tabs.Trigger
								value={p.id}
								class="flex items-center justify-center gap-2 rounded-lg text-sm font-medium data-[state=active]:bg-background data-[state=active]:shadow-craft-sm"
							>
								<Icon class="size-4" />
								{p.title}
							</Tabs.Trigger>
						{/each}
					</Tabs.List>

					{#each platforms as p}
						{@const Icon = p.icon}
						<Tabs.Content value={p.id} class="mt-8">
							<Reveal>
								<article class="glass-card relative overflow-hidden rounded-2xl p-8 sm:p-10">
									<div class="pointer-events-none absolute -right-16 -top-16 size-48 rounded-full bg-primary/5 blur-3xl"></div>

									<div class="relative flex flex-col gap-8 sm:flex-row sm:items-start sm:justify-between">
										<div>
											<span class="glass-chip grid size-12 place-items-center rounded-xl text-foreground/70">
												<Icon class="size-5" />
											</span>
											<h3 class="mt-6 text-2xl font-semibold tracking-tight">
												{p.title}
											</h3>
											<p class="mt-1.5 text-sm text-muted-foreground">
												{p.subtitle}
											</p>
										</div>

										<div class="grid w-full gap-3 sm:max-w-xs">
											{#each platformAssets[p.id] as asset, i}
												<Button
													href={asset.link ?? undefined}
													disabled={!asset.link}
													variant={i === 0 ? "default" : "secondary"}
													class={cn("w-full justify-between gap-3", !asset.link && "opacity-60")}
												>
													<span>{asset.label}</span>
													<ArrowDownToLine class="size-4 opacity-70" />
												</Button>
											{/each}
										</div>
									</div>

									<!-- macOS-only first-launch block. We're not yet Apple-notarized,
									     so Gatekeeper refuses a freshly-downloaded DMG with "is
									     damaged and can't be opened" (especially on Apple Silicon
									     where ad-hoc signatures aren't trusted). The literal error
									     text is included verbatim so search engines surface this
									     page when users paste the message into Google. -->
									{#if p.id === "macOS"}
										<div
											id="macos-first-launch"
											class="relative mt-8 rounded-2xl border border-amber-500/25 bg-amber-500/4 p-5 sm:p-6"
										>
											<div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:gap-4">
												<span
													class="grid size-9 shrink-0 place-items-center rounded-xl bg-amber-500/15 text-amber-600 dark:text-amber-400"
												>
													<TriangleAlert class="size-4" />
												</span>
												<div class="flex-1 space-y-3">
													<div>
														<h4 class="text-sm font-semibold text-foreground">
															First launch on macOS
														</h4>
														<p
															class="mt-1 text-sm leading-relaxed text-muted-foreground"
														>
															If macOS shows
															<em class="not-italic font-medium text-foreground/85"
																>"Recast" is damaged and can't be opened</em
															>, the download is fine — we're not yet
															Apple-notarized. Drag Recast.app into your
															<span class="font-mono text-foreground/90">Applications</span>
															folder, then run this once in Terminal:
														</p>
													</div>
													<pre
														class="overflow-x-auto rounded-lg border border-border-low/50 bg-foreground/4 px-3 py-2.5 font-mono text-xs text-foreground"><code>xattr -dr com.apple.quarantine /Applications/Recast.app</code></pre>
													<p class="text-xs text-muted-foreground/80">
														After that, open Recast normally from Launchpad or
														Applications. The Terminal step will go away once we
														ship a notarized build. First launch will also prompt
														for Screen Recording, Microphone, and Camera permission
														in System Settings → Privacy & Security — grant the
														ones you intend to record from.
													</p>
												</div>
											</div>
										</div>
									{/if}
								</article>
							</Reveal>
						</Tabs.Content>
					{/each}
				</Tabs.Root>
			</div>

			<div class="glass-card mt-10 flex flex-col items-start gap-3 rounded-2xl p-5 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between sm:p-6">
				<div class="flex items-center gap-2.5">
					<span class="glass-chip grid size-8 place-items-center rounded-lg text-foreground/70">
						<ShieldCheck class="size-4" />
					</span>
					<span>
						Source on
						<a
							href="https://github.com/kanakkholwal/recast"
							target="_blank"
							rel="noopener noreferrer"
							class="font-semibold text-foreground transition-colors hover:text-primary"
						>
							GitHub →
						</a>
					</span>
				</div>
				<span class="font-mono text-xs">
					Verify checksums on the
					<a
						href="https://github.com/kanakkholwal/recast/releases/latest"
						target="_blank"
						rel="noopener noreferrer"
						class="font-semibold text-foreground transition-colors hover:text-primary"
					>
						release page →
					</a>
				</span>
			</div>
		</Container>
	</Section>

	<Footer />
</main>

<style>
	.dl-aurora {
		background:
			radial-gradient(ellipse 80% 50% at 50% -10%, color-mix(in srgb, var(--color-primary) 12%, transparent), transparent 70%),
			radial-gradient(ellipse 60% 40% at 18% 8%, color-mix(in srgb, var(--color-primary) 7%, transparent), transparent 70%),
			radial-gradient(ellipse 60% 40% at 82% 8%, color-mix(in srgb, var(--color-primary) 8%, transparent), transparent 70%);
	}

	:global(.dark) .dl-aurora {
		background:
			radial-gradient(ellipse 80% 50% at 50% -10%, color-mix(in srgb, var(--color-primary) 7%, transparent), transparent 75%),
			radial-gradient(ellipse 60% 40% at 18% 8%, color-mix(in srgb, var(--color-primary) 4%, transparent), transparent 75%),
			radial-gradient(ellipse 60% 40% at 82% 8%, color-mix(in srgb, var(--color-primary) 5%, transparent), transparent 75%);
	}

	.dl-grid {
		background-image:
			linear-gradient(to right, color-mix(in srgb, var(--color-foreground) 5%, transparent) 1px, transparent 1px),
			linear-gradient(to bottom, color-mix(in srgb, var(--color-foreground) 5%, transparent) 1px, transparent 1px);
		background-size: 64px 64px;
		mask-image: radial-gradient(ellipse 70% 60% at 50% 30%, black 30%, transparent 75%);
	}

	.dl-cta {
		box-shadow:
			inset 0 1px 0 0 color-mix(in srgb, white 14%, transparent),
			inset 0 -1px 0 0 color-mix(in srgb, black 18%, transparent),
			0 1px 2px rgba(0, 0, 0, 0.06),
			0 8px 24px -8px rgba(0, 0, 0, 0.18),
			0 18px 40px -12px rgba(0, 0, 0, 0.22);
	}

	.dl-cta:hover {
		box-shadow:
			inset 0 1px 0 0 color-mix(in srgb, white 18%, transparent),
			inset 0 -1px 0 0 color-mix(in srgb, black 18%, transparent),
			0 2px 4px rgba(0, 0, 0, 0.08),
			0 14px 32px -8px rgba(0, 0, 0, 0.22),
			0 24px 56px -12px rgba(0, 0, 0, 0.28);
	}
</style>
