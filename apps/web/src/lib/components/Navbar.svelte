<script lang="ts">
	import Logo from "$lib/logo.svelte";
	import { Button } from "@recast/ui/button";

	import { cn } from "@recast/ui/utils";

	let { class: className = "" } = $props();
	let scrolled = $state(false);

	if (typeof window !== "undefined") {
		window.addEventListener(
			"scroll",
			() => {
				scrolled = window.scrollY > 20;
			},
			{ passive: true },
		);
	}
</script>

<header
	class={cn(
		"fixed top-4 left-1/2 -translate-x-1/2 z-50 transition-all duration-300 w-[95vw] lg:w-[1024px] max-w-screen-xl rounded-2xl mx-auto glass-panel border shadow-craft-md",
		scrolled ? "border-border-low/40 bg-background/60" : "bg-background/20 border-transparent",
		className,
	)}
>
	<div class="px-6 py-3.5 flex items-center justify-between relative">
		<!-- Brand -->
		<a
			href="/"
			class="flex items-center gap-3 group transition-transform active:scale-95"
		>
			<div class="size-7 flex items-center justify-center p-0.5 rounded-lg bg-foreground/5 shadow-craft-sm">
				<Logo size="24" />
			</div>
			<span class="text-base font-bold tracking-tight">Recast</span>
		</a>

		<!-- Navigation -->
		<nav
			class="hidden md:flex absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 items-center gap-8 text-[14px] font-medium text-foreground/50 tracking-wide font-serif"
		>
			<a
				href="/#workflow"
				class="hover:text-foreground transition-colors"
			>
				Capabilities
			</a>
			<a href="/#use-cases" class="hover:text-foreground transition-colors"
				>Use Cases</a
			>
			<a href="https://github.com/kanakkholwal/recast/releases" target="_blank" rel="noreferrer" class="hover:text-foreground transition-colors"
				>Changelog</a
			>
		</nav>

		<!-- Actions -->
		<div class="flex items-center gap-4">
			<a
				href="https://github.com/kanakkholwal/recast"
				target="_blank"
				rel="noreferrer"
				class="hidden md:flex text-foreground/50 hover:text-foreground transition-colors p-2"
				aria-label="GitHub Repository"
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					width="20"
					height="20"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					class="size-5"
				><path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4"/><path d="M9 18c-4.51 2-5-2-7-2"/></svg>
			</a>
			<div class="h-4 w-px bg-border-low hidden md:block" aria-hidden="true"></div>
			<Button
				variant="default"
				size="sm"
				class="text-[13px] font-bold h-9 px-5 rounded-full shadow-craft-sm bg-foreground text-background hover:bg-foreground/90 transition-all active:scale-95"
				href="/download"
			>
				Download
			</Button>
		</div>
	</div>
</header>
