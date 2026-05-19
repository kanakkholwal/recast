<script lang="ts">
	import { page } from "$app/state";
	import * as Sidebar from "@recast/ui/sidebar";

	function titleCase(s: string) {
		return s.charAt(0).toUpperCase() + s.slice(1);
	}

	// Breadcrumb derived from the route — "Settings / Profile" etc.
	const crumb = $derived.by(() => {
		const parts = page.url.pathname.split("/").filter(Boolean); // ["dashboard", ...]
		if (parts[1] === "settings") {
			return { section: "Settings", sub: parts[2] ? titleCase(parts[2]) : null };
		}
		return { section: "Recordings", sub: null };
	});
</script>

<header
	class="sticky top-0 z-20 flex h-14 shrink-0 items-center gap-2.5 border-b border-border-low/60 bg-background/80 px-4 backdrop-blur-xl"
>
	<Sidebar.Trigger
		class="size-7 rounded-md text-muted-foreground transition-colors hover:bg-foreground/6 hover:text-foreground"
		title="Toggle sidebar (⌘B)"
	/>
	<span class="h-5 w-px bg-border-low/70"></span>
	<nav class="flex items-center gap-1.5 text-sm" aria-label="Breadcrumb">
		<span class="font-semibold text-foreground">{crumb.section}</span>
		{#if crumb.sub}
			<span class="text-muted-foreground/40">/</span>
			<span class="text-muted-foreground">{crumb.sub}</span>
		{/if}
	</nav>
</header>
