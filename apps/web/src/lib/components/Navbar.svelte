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
		"fixed top-6 left-1/2 -translate-x-1/2 z-50 transition-all duration-300 w-fit max-w-[95vw]",
		className,
	)}
>
	<div
		class="glass-panel shadow-craft-md rounded-full px-6 py-3 flex items-center gap-8"
	>
		<a
			href="/"
			class="flex items-center gap-2 group transition-transform active:scale-95"
		>
			<div class="size-6 flex items-center justify-center">
				<Logo size="24" />
			</div>
			<span class="text-sm font-semibold tracking-tight">Recast</span>
		</a>

		<nav
			class="hidden md:flex items-center gap-6 text-[13px] font-medium text-foreground/60"
		>
			<a href="#pillars" class="hover:text-foreground transition-colors"
				>Philosophy</a
			>
			<a
				href="#how-it-works"
				class="hover:text-foreground transition-colors">Workflow</a
			>
			<a href="#features" class="hover:text-foreground transition-colors"
				>Features</a
			>
		</nav>

		<div class="h-4 w-px bg-border-low hidden md:block"></div>

		<div class="flex items-center gap-4">
			<Button
				variant="ghost"
				size="sm"
				class="text-[13px] font-medium h-8 px-4 rounded-full"
				href="/download"
			>
				Get Started
			</Button>
		</div>
	</div>
</header>
