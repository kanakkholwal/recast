<script lang="ts">
	import { Container, Footer, Navbar, Section } from "$lib/components";
	import { Button } from "@recast/ui/button";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import { cn } from "@recast/ui/utils";
	import { Apple, ArrowDownToLine, ChevronDown, ChevronRight, Download, Monitor, Terminal, Sparkles } from "lucide-svelte";
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

<main class="bg-background text-foreground selection:bg-primary/10 font-sans min-h-screen pt-32 pb-24 relative">
    <div class="fixed inset-0 bg-grid-pattern opacity-5 pointer-events-none mix-blend-overlay"></div>

	<Section class="py-20 md:py-32 relative z-10">
		<Container>
			<div class="max-w-4xl mx-auto flex flex-col items-center text-center">
				<div class="inline-flex items-center gap-2 px-4 py-1.5 rounded-full border border-border-low bg-muted/40 backdrop-blur-md mb-10 shadow-craft-sm hover:bg-muted/60 transition-colors cursor-default">
					<Sparkles class="size-3 text-primary" />
					<span class="text-[12px] font-bold uppercase tracking-[0.15em] text-foreground/70">Latest Release: {data.version}</span>
				</div>

				<h1 class="text-5xl md:text-7xl lg:text-[5.5rem] font-semibold tracking-tight mb-8 text-foreground leading-[1.05]">
					Get Recast for <br />
					<span class="text-foreground/40 font-serif italic">{detectedOS !== "Unknown" ? detectedOS : "Desktop"}</span>
				</h1>
				
				<p class="text-lg md:text-2xl text-foreground/50 leading-relaxed max-w-2xl mb-16 font-medium">
					Free during beta. No sign-up required. Start creating cinematic, refined screen recordings in seconds.
				</p>

				{#if primaryDownload().link}
					<div class="flex flex-col items-center gap-6 group">
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
									<DropdownMenu.Content align="end" class="w-56 p-1 rounded-xl shadow-craft-lg">
										<DropdownMenu.Label class="text-[11px] text-muted-foreground uppercase tracking-widest font-bold px-2 py-2">Other architectures</DropdownMenu.Label>
										<DropdownMenu.Separator />
										{#each secondaryDownloads() as dl}
											<DropdownMenu.Item class="cursor-pointer text-sm font-medium py-2 rounded-lg" onclick={() => { if (dl.link) window.location.href = dl.link; }}>
												{dl.label}
											</DropdownMenu.Item>
										{/each}
									</DropdownMenu.Content>
								</DropdownMenu.Root>
							{/if}
						</div>

						<a href="#all-platforms" class="text-[13px] font-bold text-foreground/30 hover:text-foreground/70 transition-colors tracking-widest uppercase mt-4">
							Not on {detectedOS}? See all platforms ↓
						</a>
					</div>
				{:else}
					<Button href="#all-platforms" size="lg" class="h-16 px-12 text-lg font-bold bg-foreground text-background rounded-2xl shadow-craft-xl hover:scale-[1.02] transition-transform">
						View all downloads
						<ChevronRight class="ml-3 size-5" />
					</Button>
				{/if}
			</div>
		</Container>
	</Section>

	<Section id="all-platforms" class="py-32 relative z-10 border-t border-border-low/50 bg-background">
		<Container>
			<div class="max-w-2xl mb-20">
				<h2 class="text-4xl md:text-5xl font-semibold text-foreground tracking-tight mb-6">Platforms.</h2>
				<p class="text-xl text-foreground/50 font-medium">Download the specific package for your architecture.</p>
			</div>

			<div class="grid grid-cols-1 md:grid-cols-3 gap-6">
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
					<div class="group relative overflow-hidden bg-background border border-border-low rounded-[2rem] p-8 shadow-craft-sm hover:shadow-craft-md transition-all duration-300 hover:-translate-y-1 invisible-ui flex flex-col">
                        <div class="absolute -right-12 -top-12 size-32 bg-foreground/5 rounded-full blur-3xl group-hover:bg-primary/5 transition-colors duration-500 pointer-events-none"></div>

						<div class="size-12 rounded-2xl bg-muted border border-border flex items-center justify-center mb-8 group-hover:scale-105 group-hover:bg-primary/5 transition-all duration-300 shadow-sm">
							<platform.icon class="size-5 text-foreground/60 group-hover:text-primary transition-colors" />
						</div>
						
						<h3 class="text-2xl font-semibold mb-2 text-foreground tracking-tight">{platform.title}</h3>
						<p class="text-sm text-foreground/40 mb-10 font-medium">{platform.subtitle}</p>
						
						<div class="mt-auto space-y-3 relative z-10">
							{#each platform.links as link}
								<Button 
									href={link.href} 
									variant={link.primary ? "default" : "secondary"}
									class={cn(
										"w-full justify-between group/link transition-all rounded-xl h-12 px-5 text-[13px] font-bold shadow-none",
                                        link.primary ? "bg-foreground text-background hover:bg-foreground/90 hover:shadow-craft-sm" : "bg-muted/50 hover:bg-muted text-foreground/80 hover:text-foreground border border-transparent hover:border-border-low"
									)} 
									disabled={!link.href}
								>
									{link.label}
									<ArrowDownToLine class={cn(
                                        "size-4 transition-opacity",
                                        link.primary ? "opacity-40 group-hover/link:opacity-100" : "opacity-30 group-hover/link:opacity-100 text-foreground"
                                    )} />
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