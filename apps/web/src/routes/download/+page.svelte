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
		Monitor,
		Sparkles,
		Terminal,
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
</script>

<svelte:head>
	<title>Download Recast — macOS, Windows, Linux</title>
	<meta
		name="description"
		content="Download Recast for macOS, Windows, or Linux. Free during beta. The intentional screen recorder."
	/>
</svelte:head>

<main class="bg-background text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-24">
		<div class="bg-aurora pointer-events-none absolute inset-0 -z-10 opacity-90"></div>
		<div class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 -z-10 opacity-50"></div>

		<Container class="relative">
			<div class="mx-auto flex max-w-3xl flex-col items-center text-center">
				<Eyebrow icon={Sparkles} variant="primary">
					Latest release · {data.version}
				</Eyebrow>

				<h1 class="text-balance mt-7 animate-fade-up text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl">
					Get Recast for
					<span class="font-medium italic text-foreground/40">
						{detectedOS !== "Unknown" ? detectedOS : "your desktop"}.
					</span>
				</h1>

				<p
					class="text-pretty mt-6 max-w-xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg"
					style="animation-delay: 120ms"
				>
					Free during beta. No sign-up. Three platforms. One opinionated tool.
				</p>

				<div
					class="mt-10 flex animate-fade-up flex-col items-center gap-3"
					style="animation-delay: 240ms"
				>
					{#if primary?.link}
						<div
							class="flex items-stretch overflow-hidden rounded-xl bg-foreground text-background shadow-craft-xl ring-1 ring-foreground/20 transition-transform hover:scale-[1.01] active:scale-[0.99]"
						>
							<a
								href={primary.link}
								class="flex h-12 items-center gap-3 px-6 text-sm font-semibold transition-colors hover:bg-background/10 sm:px-8 sm:text-base"
							>
								<Download class="size-4" />
								Download for {detectedOS}
								<span class="hidden text-xs font-medium opacity-60 sm:inline">
									· {primary.label}
								</span>
							</a>
							{#if secondary.length}
								<DropdownMenu.Root>
									<DropdownMenu.Trigger
										class="grid w-12 place-items-center border-l border-background/15 transition-colors hover:bg-background/10"
										aria-label="Other architectures"
									>
										<ChevronDown class="size-4 opacity-80" />
									</DropdownMenu.Trigger>
									<DropdownMenu.Content align="end" class="w-60 rounded-xl p-1 shadow-craft-lg">
										<DropdownMenu.Label class="px-2 py-1.5 text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
											Other architectures
										</DropdownMenu.Label>
										<DropdownMenu.Separator />
										{#each secondary as opt}
											<DropdownMenu.Item
												class="cursor-pointer rounded-lg py-2 text-sm font-medium"
												onclick={() => opt.link && (window.location.href = opt.link)}
											>
												{opt.label}
											</DropdownMenu.Item>
										{/each}
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
				</div>
			</div>
		</Container>
	</Section>

	<Section id="all-platforms" class="border-t border-border-low/60 bg-background">
		<Container>
			<SectionHeader
				eyebrow="All platforms"
				title="Pick your build."
				description="Native binaries for every supported platform and architecture."
			/>

			<div class="mt-12">
				<Tabs.Root value={activeTab} class="w-full">
					<Tabs.List class="grid w-full grid-cols-3 rounded-xl border border-border-low bg-card/60 p-1 sm:max-w-md">
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
								<article class="relative overflow-hidden rounded-2xl border border-border-low bg-card/60 p-8 sm:p-10">
									<div class="pointer-events-none absolute -right-16 -top-16 size-48 rounded-full bg-primary/5 blur-3xl"></div>

									<div class="relative flex flex-col gap-8 sm:flex-row sm:items-start sm:justify-between">
										<div>
											<span class="grid size-12 place-items-center rounded-xl border border-border-low bg-background/80 text-foreground/70">
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
								</article>
							</Reveal>
						</Tabs.Content>
					{/each}
				</Tabs.Root>
			</div>

			<div class="mt-10 flex flex-col items-start gap-2 rounded-2xl border border-border-low bg-muted/30 p-5 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between">
				<span>
					Source on
					<a
						href="https://github.com/kanakkholwal/recast"
						target="_blank"
						rel="noopener noreferrer"
						class="font-semibold text-foreground hover:text-primary"
					>
						GitHub →
					</a>
				</span>
				<span class="font-mono text-xs">
					Verify checksums on the
					<a
						href="https://github.com/kanakkholwal/recast/releases/latest"
						target="_blank"
						rel="noopener noreferrer"
						class="font-semibold text-foreground hover:text-primary"
					>
						release page
					</a>
				</span>
			</div>
		</Container>
	</Section>

	<Footer />
</main>
