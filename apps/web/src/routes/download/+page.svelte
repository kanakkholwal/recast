<script lang="ts">
	import { Container, Navbar, Section } from "$lib/components";
	import { Button } from "@recast/ui/button";
	import { Apple, ArrowDownToLine, ChevronRight, Download, Monitor, Terminal } from "lucide-svelte";
	import type { PageData } from "./$types";

	let { data }: { data: PageData } = $props();

	// Client-side OS detection for the primary hero CTA
	let detectedOS = $state<"macOS" | "Windows" | "Linux" | "Unknown">("Unknown");

	$effect(() => {
		const ua = window.navigator.userAgent;
		if (ua.includes("Mac")) detectedOS = "macOS";
		else if (ua.includes("Win")) detectedOS = "Windows";
		else if (ua.includes("Linux")) detectedOS = "Linux";
	});

	// Helper to resolve the best direct download link based on OS
	let primaryDownload = $derived(() => {
		switch (detectedOS) {
			case "macOS":
				return { link: data.downloads.macosAppleSilicon, label: "Download for Mac (Apple Silicon)" };
			case "Windows":
				return { link: data.downloads.windowsExe || data.downloads.windowsMsi, label: "Download for Windows" };
			case "Linux":
				return { link: data.downloads.linuxAppImage || data.downloads.linuxDeb, label: "Download for Linux" };
			default:
				return { link: null, label: "Select your platform below" };
		}
	});
</script>

<svelte:head>
	<title>Download Recast — Record. Refine. Share.</title>
	<meta name="description" content="Download Recast for macOS, Windows, or Linux. The intentional screen recorder." />
</svelte:head>

<Navbar />

<main class="bg-background text-foreground min-h-screen pt-24 pb-16">
	<Section class="py-16 md:py-24 text-center">
		<Container>
			<div class="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-muted/50 border border-border/50 text-sm font-medium text-muted-foreground mb-8">
				<div class="size-2 rounded-full bg-primary/80 animate-pulse"></div>
				Latest Release: {data.version}
			</div>

			<h1 class="text-4xl md:text-6xl font-semibold tracking-tight mb-6">
				Get Recast for {detectedOS !== "Unknown" ? detectedOS : "Desktop"}
			</h1>
			<p class="text-lg text-muted-foreground max-w-xl mx-auto mb-10 text-balance">
				Free for local use. No sign-up required. Start creating cinematic, refined screen recordings in seconds.
			</p>

			{#if primaryDownload().link}
				<div class="flex flex-col items-center gap-4">
					<Button href={primaryDownload().link} size="lg" class="h-14 px-8 text-base font-medium shadow-sm group rounded-full">
						<Download class="mr-2 size-5 group-hover:-translate-y-0.5 transition-transform" />
						{primaryDownload().label}
					</Button>
					<a href="#all-platforms" class="text-sm text-muted-foreground hover:text-foreground font-medium transition-colors">
						Not on {detectedOS}? See all platforms ↓
					</a>
				</div>
			{:else}
				<Button href="#all-platforms" size="lg" variant="default" class="h-14 text-base font-medium shadow-sm rounded-full">
					View all downloads
					<ChevronRight class="ml-2 size-4" />
				</Button>
			{/if}
		</Container>
	</Section>

	<Section id="all-platforms" class="py-16 md:py-24 border-t border-border/40 bg-muted/10">
		<Container>
			<div class="mb-12">
				<h2 class="text-2xl font-semibold tracking-tight mb-2">All Platforms</h2>
				<p class="text-muted-foreground">Download the specific package for your architecture.</p>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
				<div class="p-8 rounded-[2rem] bg-background border border-border/50 flex flex-col hover:border-border transition-colors">
					<div class="size-12 rounded-xl bg-muted flex items-center justify-center mb-6">
						<Apple class="size-6 text-foreground" />
					</div>
					<h3 class="text-xl font-semibold mb-2">macOS</h3>
					<p class="text-sm text-muted-foreground mb-8">Requires macOS 12.0 or later.</p>
					
					<div class="mt-auto space-y-3">
						<Button href={data.downloads.macosAppleSilicon} variant="secondary" class="w-full justify-between group" disabled={!data.downloads.macosAppleSilicon}>
							Apple Silicon (.dmg)
							<ArrowDownToLine class="size-4 opacity-50 group-hover:opacity-100 transition-opacity" />
						</Button>
						<Button href={data.downloads.macosIntel} variant="outline" class="w-full justify-between group" disabled={!data.downloads.macosIntel}>
							Intel (.dmg)
							<ArrowDownToLine class="size-4 opacity-50 group-hover:opacity-100 transition-opacity" />
						</Button>
					</div>
				</div>

				<div class="p-8 rounded-[2rem] bg-background border border-border/50 flex flex-col hover:border-border transition-colors">
					<div class="size-12 rounded-xl bg-muted flex items-center justify-center mb-6">
						<Monitor class="size-6 text-foreground" />
					</div>
					<h3 class="text-xl font-semibold mb-2">Windows</h3>
					<p class="text-sm text-muted-foreground mb-8">Requires Windows 10 or later.</p>
					
					<div class="mt-auto space-y-3">
						<Button href={data.downloads.windowsExe} variant="secondary" class="w-full justify-between group" disabled={!data.downloads.windowsExe}>
							Installer (.exe)
							<ArrowDownToLine class="size-4 opacity-50 group-hover:opacity-100 transition-opacity" />
						</Button>
						<Button href={data.downloads.windowsMsi} variant="outline" class="w-full justify-between group" disabled={!data.downloads.windowsMsi}>
							Installer (.msi)
							<ArrowDownToLine class="size-4 opacity-50 group-hover:opacity-100 transition-opacity" />
						</Button>
					</div>
				</div>

				<div class="p-8 rounded-[2rem] bg-background border border-border/50 flex flex-col hover:border-border transition-colors">
					<div class="size-12 rounded-xl bg-muted flex items-center justify-center mb-6">
						<Terminal class="size-6 text-foreground" />
					</div>
					<h3 class="text-xl font-semibold mb-2">Linux</h3>
					<p class="text-sm text-muted-foreground mb-8">Debian, Ubuntu, and Red Hat distributions.</p>
					
					<div class="mt-auto space-y-3">
						<Button href={data.downloads.linuxAppImage} variant="secondary" class="w-full justify-between group" disabled={!data.downloads.linuxAppImage}>
							AppImage
							<ArrowDownToLine class="size-4 opacity-50 group-hover:opacity-100 transition-opacity" />
						</Button>
						<div class="grid grid-cols-2 gap-3">
							<Button href={data.downloads.linuxDeb} variant="outline" class="w-full justify-between group px-3" disabled={!data.downloads.linuxDeb}>
								.deb
								<ArrowDownToLine class="size-4 opacity-50 group-hover:opacity-100 transition-opacity" />
							</Button>
							<Button href={data.downloads.linuxRpm} variant="outline" class="w-full justify-between group px-3" disabled={!data.downloads.linuxRpm}>
								.rpm
								<ArrowDownToLine class="size-4 opacity-50 group-hover:opacity-100 transition-opacity" />
							</Button>
						</div>
					</div>
				</div>
			</div>
		</Container>
	</Section>
</main>