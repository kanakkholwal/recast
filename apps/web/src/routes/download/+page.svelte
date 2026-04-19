<script lang="ts">
	import { Container, Footer, Navbar, Section } from "$lib/components";
	import { Button } from "@recast/ui/button";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { cn } from "@recast/ui/utils";
	import { Apple, ArrowDownToLine, ChevronDown, ChevronRight, Download, Monitor, Terminal } from "lucide-svelte";
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

	// Helper to resolve OS-specific downloads into a primary and an array of secondaries
	let platformLinks = $derived(() => {
		switch (detectedOS) {
			case "macOS":
				return [
					{ link: data.downloads.macosAppleSilicon, label: "Apple Silicon (.dmg)" },
					{ link: data.downloads.macosIntel, label: "Intel (.dmg)" }
				];
			case "Windows":
				return [
					{ link: data.downloads.windowsExe, label: "Installer (.exe)" },
					{ link: data.downloads.windowsMsi, label: "Installer (.msi)" }
				];
			case "Linux":
				return [
					{ link: data.downloads.linuxAppImage, label: "AppImage (Universal)" },
					{ link: data.downloads.linuxDeb, label: "Debian/Ubuntu (.deb)" },
					{ link: data.downloads.linuxRpm, label: "Red Hat/Fedora (.rpm)" }
				];
			default:
				return [];
		}
	});

	let primaryDownload = $derived(() => {
		const links = platformLinks();
		return links.length > 0 ? links[0] : { link: null, label: "Select your platform below" };
	});

	let secondaryDownloads = $derived(() => {
		const links = platformLinks();
		return links.length > 1 ? links.slice(1) : [];
	});
</script>

<svelte:head>
	<title>Download Recast — Record. Refine. Share.</title>
	<meta name="description" content="Download Recast for macOS, Windows, or Linux. The intentional screen recorder." />
</svelte:head>

<Navbar />

<main class="bg-background text-foreground/80 min-h-screen pt-32 pb-24">
	<Section class="py-16 md:py-24">
		<Container>
			<div class="max-w-6xl">
				<div class="inline-flex items-center gap-3 px-4 py-1.5 rounded-full glass-panel shadow-craft-sm text-[13px] font-semibold text-foreground/40 mb-12">
					<div class="size-2 rounded-full bg-success animate-pulse"></div>
					Latest Release: {data.version}
				</div>

				<h1 class="text-6xl md:text-8xl font-semibold tracking-[-0.03em] mb-10 text-foreground">
					Get Recast for <br />
					<span class="text-foreground/30 font-medium italic">{detectedOS !== "Unknown" ? detectedOS : "Desktop"}</span>
				</h1>
				
				<p class="text-xl md:text-2xl text-foreground/50 leading-relaxed max-w-2xl mb-16 font-medium">
					Free for local use. No sign-up required. Start creating cinematic, refined screen recordings in seconds.
				</p>

				{#if primaryDownload().link}
					<div class="flex flex-col items-start gap-8 group">
						<div class="flex items-stretch bg-foreground text-background shadow-craft-xl rounded-2xl transition-all hover:scale-[1.02] active:scale-95 divide-x divide-background/20 overflow-hidden">
							<a href={primaryDownload().link || '#'} class="flex items-center h-16 px-10 text-lg font-bold hover:bg-background/10 transition-colors focus-visible:outline-none">
								<Download class="mr-3 size-6" />
								Download for {detectedOS}
								<span class="ml-2 text-sm font-medium opacity-60">({primaryDownload().label})</span>
							</a>
							
							{#if secondaryDownloads().length > 0}
								<DropdownMenu.Root>
									<DropdownMenu.Trigger class="w-14 items-center justify-center flex hover:bg-background/10 transition-colors focus-visible:outline-none">
										<ChevronDown class="size-6 opacity-80" />
									</DropdownMenu.Trigger>
									<DropdownMenu.Content align="end" class="w-56 p-1">
										<DropdownMenu.Label class="text-xs text-muted-foreground uppercase tracking-widest font-bold px-2 py-1.5">Other architectures</DropdownMenu.Label>
										<DropdownMenu.Separator />
										{#each secondaryDownloads() as dl}
											<DropdownMenu.Item class="cursor-pointer" onclick={() => { if (dl.link) window.location.href = dl.link; }}>
												{dl.label}
											</DropdownMenu.Item>
										{/each}
									</DropdownMenu.Content>
								</DropdownMenu.Root>
							{/if}
						</div>

						<a href="#all-platforms" class="text-sm font-bold text-foreground/20 hover:text-foreground/60 transition-colors tracking-wide uppercase">
							Not on {detectedOS}? See all platforms ↓
						</a>
					</div>
				{:else}
					<Button href="#all-platforms" size="lg" class="h-16 px-12 text-lg font-bold bg-foreground text-background rounded-2xl shadow-craft-xl">
						View all downloads
						<ChevronRight class="ml-3 size-5" />
					</Button>
				{/if}
			</div>
		</Container>
	</Section>

	<Section id="all-platforms" class="py-24 bg-muted/30 dark:bg-white/2 border-y border-border-low">
		<Container>
			<div class="max-w-2xl mb-24">
				<h2 class="text-5xl font-semibold text-foreground tracking-tight mb-6">All Platforms.</h2>
				<p class="text-xl text-foreground/50">Download the specific package for your architecture.</p>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-3 gap-8">
				{#each [
					{ 
						id: 'macos', 
						icon: Apple, 
						title: 'macOS', 
						subtitle: 'Requires macOS 12.0+', 
						links: [
							{ label: 'Apple Silicon (.dmg)', href: data.downloads.macosAppleSilicon, primary: true },
							{ label: 'Intel (.dmg)', href: data.downloads.macosIntel }
						] 
					},
					{ 
						id: 'windows', 
						icon: Monitor, 
						title: 'Windows', 
						subtitle: 'Requires Windows 10+', 
						links: [
							{ label: 'Installer (.exe)', href: data.downloads.windowsExe, primary: true },
							{ label: 'Installer (.msi)', href: data.downloads.windowsMsi }
						] 
					},
					{ 
						id: 'linux', 
						icon: Terminal, 
						title: 'Linux', 
						subtitle: 'Debian, Ubuntu, Red Hat', 
						links: [
							{ label: 'AppImage', href: data.downloads.linuxAppImage, primary: true },
							{ label: '.deb', href: data.downloads.linuxDeb },
							{ label: '.rpm', href: data.downloads.linuxRpm }
						] 
					}
				] as platform}
					<div class="group craft-block bg-white dark:bg-neutral-900 border border-border-low shadow-craft-sm hover:shadow-craft-md flex flex-col">
						<div class="size-12 rounded-2xl bg-muted/50 dark:bg-white/5 flex items-center justify-center mb-8 group-hover:scale-110 transition-transform">
							<platform.icon class="size-5 text-foreground/60" />
						</div>
						
						<h3 class="text-2xl font-semibold mb-2 text-foreground">{platform.title}</h3>
						<p class="text-sm text-foreground/40 mb-10 font-medium">{platform.subtitle}</p>
						
						<div class="mt-auto space-y-3">
							{#each platform.links as link}
								<Button 
									href={link.href} 
									variant="ghost" 
									class={cn(
										"w-full justify-between group/link transition-all rounded-xl h-12 px-5 text-[13px] font-bold",
									)} 
									disabled={!link.href}
								>
									{link.label}
									<ArrowDownToLine class="size-4 opacity-20 group-hover/link:opacity-100 transition-opacity" />
								</Button>
							{/each}
						</div>
					</div>
				{/each}
			</div>
		</Container>
	</Section>
</main>

<Footer />