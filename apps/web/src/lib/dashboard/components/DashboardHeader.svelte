<script lang="ts">
	import { page } from "$app/state";
	import CommandPaletteHost from "$lib/dashboard/components/CommandPaletteHost.svelte";
	import SearchTrigger from "$lib/dashboard/components/SearchTrigger.svelte";
	import UserMenu from "$lib/dashboard/components/UserMenu.svelte";
	import * as Sidebar from "@recast/ui/sidebar";

	interface Props {
		/** Label shown at the route root (e.g. "Home" for /dashboard). */
		rootLabel?: string;
		/** Pretty names for second-segment routes; falls back to title-case. */
		labels?: Record<string, string>;
	}

	let { rootLabel = "Home", labels }: Props = $props();

	function titleCase(s: string) {
		return s.charAt(0).toUpperCase() + s.slice(1);
	}

	const sectionMap = $derived(
		labels ?? {
			recasts: "Recasts",
			analytics: "Analytics",
			settings: "Settings",
		},
	);

	// Breadcrumb derived from the route — "Settings / Profile" etc.
	const crumb = $derived.by(() => {
		const parts = page.url.pathname.split("/").filter(Boolean); // ["dashboard", ...]
		if (parts.length <= 1) return { section: rootLabel, sub: null };
		const second = parts[1]!;
		const section = sectionMap[second] ?? titleCase(second);
		// Only settings has deeper routes (Profile / Integrations / Preferences).
		const sub = second === "settings" && parts[2] ? titleCase(parts[2]) : null;
		return { section, sub };
	});

	// On the dashboard home the search lives in the hero, so the header trigger
	// stands down there to avoid two search bars on one screen.
	const isHome = $derived(page.url.pathname === "/dashboard");
</script>

<!-- The one shared command dialog + ⌘K, mounted once for every dashboard/admin
	 route. Header and hero triggers both open THIS. -->
<CommandPaletteHost />

<!-- Sticky inside the inset panel: on md+ the panel is inset by `m-2`, so the
	 header sticks at `top-2` and rounds its top corners to match the panel; on
	 mobile the panel is full-bleed, so it sticks flush at `top-0` with square
	 corners. -->
<header
	class="sticky top-0 z-20 flex h-14 shrink-0 items-center gap-2.5 border-b border-border-low/60 bg-background/80 px-4 backdrop-blur-xl md:top-2 md:rounded-t-xl"
>
	<Sidebar.Trigger
		class="size-7 shrink-0 rounded-md text-muted-foreground transition-colors hover:bg-foreground/6 hover:text-foreground"
		title="Toggle sidebar (⌘B)"
	/>
	<span class="h-5 w-px shrink-0 bg-border-low/70"></span>
	<nav class="hidden shrink-0 items-center gap-1.5 text-sm sm:flex" aria-label="Breadcrumb">
		<span class="font-semibold text-foreground">{crumb.section}</span>
		{#if crumb.sub}
			<span class="text-muted-foreground/40">/</span>
			<span class="text-muted-foreground">{crumb.sub}</span>
		{/if}
	</nav>

	<!-- Search takes the flexible middle; capped so it doesn't sprawl on wide
		 viewports. On the home route it's hidden (the hero owns it), but the
		 spacer stays so the profile menu keeps its right-edge anchor. -->
	<div class="mx-auto flex w-full max-w-md items-center">
		{#if !isHome}
			<SearchTrigger />
		{/if}
	</div>

	<UserMenu />
</header>
