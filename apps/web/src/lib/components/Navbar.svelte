<script lang="ts">
	import Logo from "$lib/logo.svelte";
	import { Button } from "@recast/ui/button";
	import { cn } from "@recast/utils";
	import { Container } from ".";

	let { class: className = "" } = $props();
	let scrolled = $state(false);

	if (typeof window !== "undefined") {
		window.addEventListener("scroll", () => {
			scrolled = window.scrollY > 20;
		}, { passive: true });
	}
</script>

<header
	class={cn(
		"fixed top-0 left-0 right-0 z-50 transition-all duration-200 border-b",
		scrolled ? "bg-background/80 backdrop-blur-md border-border/40 py-4" : "bg-transparent border-transparent py-6",
		className
	)}
>
	<Container class="flex items-center justify-between">
		<a href="/" class="flex items-center gap-2">
			<div class="size-6 flex items-center justify-center">
				<Logo size="24" />
			</div>
			<span class="text-lg font-semibold tracking-tight">Recast</span>
		</a>

		<nav class="hidden md:flex items-center gap-8 text-sm font-medium text-muted-foreground">
			<a href="#pillars" class="hover:text-foreground transition-colors">Philosophy</a>
			<a href="#how-it-works" class="hover:text-foreground transition-colors">Workflow</a>
			<a href="#features" class="hover:text-foreground transition-colors">Features</a>
		</nav>

		<div class="flex items-center gap-4">
			<Button size="sm" class="font-medium shadow-sm shadow-primary/20 hover:shadow-primary/40" href="/download">Get Started</Button>
		</div>
	</Container>
</header>