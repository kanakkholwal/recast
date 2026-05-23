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
	import * as Collapsible from "@recast/ui/collapsible";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import * as Tabs from "@recast/ui/tabs";
	import { cn } from "@recast/ui/utils";
	import {
	  Apple,
	  ArrowDownToLine,
	  CheckCircle2,
	  ChevronDown,
	  Download,
	  FileBox,
	  Info,
	  LifeBuoy,
	  Monitor,
	  ShieldCheck,
	  Sparkles,
	  Terminal,
	  TriangleAlert,
	  WifiOff,
	  Zap,
	} from "@lucide/svelte";
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

	// Per-platform shipping confidence. Surfaces the honest state of the
	// builds: Windows is the daily-driver, macOS/Linux are early ports. The
	// global heads-up card below the hero plus the per-tab chip both read
	// from this so the messaging stays in sync.
	type Stability = "stable" | "beta";
	const platforms: Array<{
		id: Exclude<OS, "Unknown">;
		icon: typeof Apple;
		title: string;
		subtitle: string;
		stability: Stability;
	}> = [
		{
			id: "macOS",
			icon: Apple,
			title: "macOS",
			subtitle: "Requires macOS 12.0 or later",
			stability: "beta",
		},
		{
			id: "Windows",
			icon: Monitor,
			title: "Windows",
			subtitle: "Requires Windows 10 or later",
			stability: "stable",
		},
		{
			id: "Linux",
			icon: Terminal,
			title: "Linux",
			subtitle: "Debian, Ubuntu, Fedora, Arch",
			stability: "beta",
		},
	];

	const stabilityCopy: Record<
		Stability,
		{ label: string; dot: string; chip: string }
	> = {
		stable: {
			label: "Stable",
			dot: "bg-emerald-500",
			chip:
				"bg-emerald-500/10 text-emerald-600 ring-emerald-500/20 dark:text-emerald-400",
		},
		beta: {
			label: "Beta · expect rough edges",
			dot: "bg-amber-500",
			chip:
				"bg-amber-500/10 text-amber-600 ring-amber-500/20 dark:text-amber-400",
		},
	};

	const ISSUES_URL = "https://github.com/kanakkholwal/recast/issues/new";

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

	// Per-platform install instructions. Step `code` is a copy-paste-ready
	// shell command; `hint` is small print rendered under the body.
	type InstallStep = { title: string; body: string; code?: string; hint?: string };
	type Faq = { title: string; body: string; code?: string };
	type PlatformGuide = { intro: string; steps: InstallStep[]; faqs: Faq[] };

	const installSteps: Record<Exclude<OS, "Unknown">, PlatformGuide> = {
		macOS: {
			intro:
				"We're not yet Apple-notarized, so Gatekeeper needs one Terminal command on first launch. After that, Recast opens normally from Launchpad.",
			steps: [
				{
					title: "Pick the right build",
					body:
						"Apple Silicon for M1/M2/M3/M4 Macs. Intel for older models. Check via   → About This Mac if you're unsure.",
				},
				{
					title: "Drag Recast into Applications",
					body:
						"Open the downloaded .dmg, then drag Recast.app into your Applications folder. Eject the disk image when you're done.",
				},
				{
					title: "Clear the quarantine attribute",
					body:
						"Open Terminal and run this once. macOS will trust Recast afterwards.",
					code: "xattr -dr com.apple.quarantine /Applications/Recast.app",
					hint: "Disappears as soon as we ship a notarized build.",
				},
				{
					title: "Grant capture permissions",
					body:
						"On first launch, System Settings → Privacy & Security will prompt for Screen Recording, Microphone, and Camera. Enable the ones you intend to record from.",
				},
			],
			faqs: [
				{
					title: "“Recast is damaged and can't be opened”",
					body:
						"Not actually damaged — that's the un-notarized Gatekeeper error. Step 3 above fixes it.",
				},
				{
					title: "Permissions don't stick after enabling",
					body:
						"Fully quit Recast (⌘Q), toggle the permission off and on under Privacy & Security, then relaunch.",
				},
			],
		},
		Windows: {
			intro:
				"Windows SmartScreen flags new publishers as 'Unknown'. One click past the warning and you're in — this goes away once we sign with an EV certificate.",
			steps: [
				{
					title: "Pick the right installer",
					body:
						".exe is the typical install. Use the .msi if your IT department's group policy requires MSI packages.",
				},
				{
					title: "Run the installer",
					body:
						"Double-click the downloaded file. If UAC asks, allow it to run.",
				},
				{
					title: "Bypass SmartScreen",
					body:
						"If you see 'Windows protected your PC', click More info, then Run anyway. The publisher will read as 'Unknown' until we add code signing.",
				},
				{
					title: "Finish setup",
					body:
						"Pick an install location and let the wizard finish. Recast launches from the Start menu — pin it to the taskbar while you're at it.",
				},
			],
			faqs: [
				{
					title: "Antivirus flags Recast as suspicious",
					body:
						"False positive — fresh unsigned binaries trip heuristic scanners until they age. Add Recast.exe to your antivirus's allowlist.",
				},
				{
					title: "Capture is empty or black",
					body:
						"Update your GPU drivers (NVIDIA/AMD/Intel) and make sure Recast isn't running in compatibility mode. Right-click the shortcut → Properties → Compatibility → uncheck everything.",
				},
			],
		},
		Linux: {
			intro:
				"Three packages cover most distros. Pick by your package manager — AppImage works on anything.",
			steps: [
				{
					title: "Pick your package",
					body:
						"AppImage = portable (any distro, no install). .deb = Debian, Ubuntu, Mint. .rpm = Fedora, RHEL, openSUSE.",
				},
				{
					title: "AppImage — mark executable & run",
					body:
						"Give it execute permission, then double-click or run from the terminal.",
					code: "chmod +x Recast-*.AppImage\n./Recast-*.AppImage",
					hint: "Some distros need libfuse2: sudo apt install libfuse2.",
				},
				{
					title: ".deb — install with apt",
					body: "apt resolves any missing dependencies for you.",
					code: "sudo apt install ./recast_*.deb",
				},
				{
					title: ".rpm — install with dnf",
					body: "Use zypper on openSUSE: sudo zypper install ./recast-*.rpm.",
					code: "sudo dnf install ./recast-*.rpm",
				},
				{
					title: "Wayland — enable the portal",
					body:
						"Recast uses xdg-desktop-portal for screen capture under Wayland. Most distros bundle it; if capture is empty, install the portal and the matching backend (GNOME or KDE).",
					code: "sudo apt install xdg-desktop-portal xdg-desktop-portal-gnome",
				},
			],
			faqs: [
				{
					title: "AppImage won't launch",
					body:
						"Missing FUSE library on newer Ubuntu / Debian.",
					code: "sudo apt install libfuse2",
				},
				{
					title: "No audio device shows up",
					body:
						"Recast captures via PipeWire. Make sure pipewire-pulse is installed so your default sink is exposed.",
					code: "sudo apt install pipewire-pulse",
				},
			],
		},
	};
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

			<!-- Honest platform-stability heads-up. Windows is the build I daily-
			     drive; macOS and Linux are early ports that haven't seen the same
			     mileage. Surface it before the user clicks any download so the
			     expectation is set up-front and the GitHub issues link is right
			     there when they hit something. -->
			<Reveal>
				<div
					class="mx-auto mt-16 max-w-3xl rounded-2xl border border-amber-500/25 bg-amber-500/[0.04] p-5 backdrop-blur-md sm:p-6"
				>
					<div class="flex items-start gap-4">
						<span
							class="grid size-10 shrink-0 place-items-center rounded-xl bg-amber-500/15 text-amber-600 ring-1 ring-amber-500/20 dark:text-amber-400"
						>
							<TriangleAlert class="size-4" />
						</span>
						<div class="min-w-0 flex-1">
							<h3 class="text-sm font-semibold tracking-tight text-foreground">
								Heads up — platform stability
							</h3>
							<p class="mt-1.5 text-sm leading-relaxed text-muted-foreground">
								Windows is the build I use daily and the most polished today.
								<span class="font-semibold text-foreground/85">
									macOS and Linux are early ports
								</span>
								— please don't expect feature parity yet, and reach for the
								Windows build if you have the choice.
							</p>
							<div class="mt-3 flex flex-wrap items-center gap-1.5">
								{#each platforms as p}
									{@const s = stabilityCopy[p.stability]}
									<span
										class={cn(
											"inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 font-mono text-[10.5px] font-semibold uppercase tracking-[0.12em] ring-1 ring-inset",
											s.chip,
										)}
									>
										<span class={cn("size-1.5 rounded-full", s.dot)}></span>
										{p.title} · {p.stability === "stable" ? "Stable" : "Beta"}
									</span>
								{/each}
							</div>
							<p class="mt-3 text-xs leading-relaxed text-muted-foreground">
								Hit a bug or papercut? Please file it on
								<a
									href={ISSUES_URL}
									target="_blank"
									rel="noopener noreferrer"
									class="font-semibold text-foreground underline decoration-foreground/30 decoration-1 underline-offset-2 transition-colors hover:text-primary hover:decoration-primary/60"
								>
									GitHub Issues
								</a>
								— I read every one and reply personally.
							</p>
						</div>
					</div>
				</div>
			</Reveal>

			<!-- Ships with every build -->
			<Reveal>
				<div class="mx-auto mt-12 grid max-w-4xl grid-cols-2 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-4">
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
						{@const guide = installSteps[p.id]}
						{@const anchorId = p.id === "macOS" ? "macos-first-launch" : `install-${p.id.toLowerCase()}`}
						{@const stab = stabilityCopy[p.stability]}
						<Tabs.Content value={p.id} class="mt-8">
							<Reveal>
								<article class="glass-card relative overflow-hidden rounded-2xl p-8 sm:p-10">
									<div class="pointer-events-none absolute -right-16 -top-16 size-48 rounded-full bg-primary/5 blur-3xl"></div>

									<div class="relative flex flex-col gap-8 sm:flex-row sm:items-start sm:justify-between">
										<div>
											<span class="glass-chip grid size-12 place-items-center rounded-xl text-foreground/70">
												<Icon class="size-5" />
											</span>
											<div class="mt-6 flex flex-wrap items-center gap-3">
												<h3 class="text-2xl font-semibold tracking-tight">
													{p.title}
												</h3>
												<span
													class={cn(
														"inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-[0.12em] ring-1 ring-inset",
														stab.chip,
													)}
													title={p.stability === "stable"
														? "This is the build I use daily."
														: "Early port — please file issues on GitHub when something breaks."}
												>
													<span class={cn("size-1.5 rounded-full", stab.dot)}></span>
													{p.stability === "stable" ? "Stable" : "Beta"}
												</span>
											</div>
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

									<!-- Setup steps — same shape for every platform so adding a new
									     OS is a data change, not a layout one. macOS preserves the
									     `#macos-first-launch` anchor that the hero CTA links to. -->
									<div id={anchorId} class="relative mt-10">
										<div class="flex flex-wrap items-center justify-between gap-3">
											<div class="flex items-center gap-2.5">
												<span
													class="grid size-8 place-items-center rounded-lg bg-foreground/5 text-foreground/70 ring-1 ring-foreground/5"
												>
													<Info class="size-4" />
												</span>
												<h4 class="text-sm font-semibold tracking-tight text-foreground">
													Install on {p.title}
												</h4>
											</div>
											<span class="font-mono text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
												{guide.steps.length} steps
											</span>
										</div>
										<p class="mt-3 max-w-2xl text-sm leading-relaxed text-muted-foreground">
											{guide.intro}
										</p>

										<ol class="relative mt-6 space-y-3">
											{#each guide.steps as step, idx}
												<li
													class="group/step relative flex gap-4 rounded-2xl border border-border-low/50 bg-foreground/1.5 p-4 transition-colors hover:bg-foreground/3 sm:p-5"
												>
													<span
														class="relative z-10 grid size-8 shrink-0 place-items-center rounded-lg bg-foreground text-background font-mono text-[12px] font-semibold tabular-nums shadow-craft-sm"
													>
														{idx + 1}
													</span>
													<div class="min-w-0 flex-1 space-y-2.5">
														<div class="text-sm font-semibold tracking-tight text-foreground">
															{step.title}
														</div>
														<p class="text-sm leading-relaxed text-muted-foreground">
															{step.body}
														</p>
														{#if step.code}
															<pre
																class="overflow-x-auto rounded-lg border border-border-low/60 bg-foreground/4 px-3 py-2.5 font-mono text-xs leading-relaxed text-foreground"><code>{step.code}</code></pre>
														{/if}
														{#if step.hint}
															<p class="flex items-start gap-1.5 text-[11px] leading-relaxed text-muted-foreground/80">
																<CheckCircle2 class="mt-0.5 size-3 shrink-0 text-primary/70" />
																<span>{step.hint}</span>
															</p>
														{/if}
													</div>
												</li>
											{/each}
										</ol>

										<!-- Troubleshooting — surfaces the common Google searches
										     ("Recast is damaged", "AppImage won't launch") so users
										     don't bounce out to file an issue. -->
										{#if guide.faqs.length}
											<div class="mt-8">
												<div class="flex items-center gap-2.5">
													<span
														class="grid size-8 place-items-center rounded-lg bg-amber-500/15 text-amber-600 ring-1 ring-amber-500/15 dark:text-amber-400"
													>
														<LifeBuoy class="size-4" />
													</span>
													<h4 class="text-sm font-semibold tracking-tight text-foreground">
														If something goes wrong
													</h4>
												</div>
												<div class="mt-4 grid gap-3 sm:grid-cols-2">
													{#each guide.faqs as faq}
														<Collapsible.Root
															class="group/faq rounded-xl border border-border-low/50 bg-foreground/1.5 p-4 transition-colors hover:bg-foreground/3 data-[state=open]:border-border-low/70"
														>
															<Collapsible.Trigger
																class="flex w-full cursor-pointer items-start justify-between gap-3 text-left"
															>
																<span class="text-sm font-medium text-foreground">
																	{faq.title}
																</span>
																<ChevronDown
																	class="mt-0.5 size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-data-[state=open]/faq:rotate-180"
																/>
															</Collapsible.Trigger>
															<Collapsible.Content>
																<div class="mt-3 space-y-2.5">
																	<p class="text-sm leading-relaxed text-muted-foreground">
																		{faq.body}
																	</p>
																	{#if faq.code}
																		<pre
																			class="overflow-x-auto rounded-lg border border-border-low/60 bg-foreground/4 px-3 py-2.5 font-mono text-xs leading-relaxed text-foreground"><code>{faq.code}</code></pre>
																	{/if}
																</div>
															</Collapsible.Content>
														</Collapsible.Root>
													{/each}
												</div>
											</div>
										{/if}

										{#if p.id === "macOS"}
											<div
												class="mt-6 flex items-start gap-3 rounded-xl border border-amber-500/25 bg-amber-500/4 p-4 text-xs leading-relaxed text-muted-foreground"
											>
												<TriangleAlert class="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-400" />
												<span>
													<span class="font-semibold text-foreground">Heads up:</span>
													until we ship Apple notarization, step 3 above is required
													— pasting <span class="font-mono text-foreground/85">"Recast is damaged"</span>
													into Google brought you here.
												</span>
											</div>
										{/if}
									</div>
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
