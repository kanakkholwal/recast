<script lang="ts">
	import { page } from "$app/state";
	import { BarChart3, Clapperboard } from "@lucide/svelte";
	import { cn } from "@recast/ui/utils";
	import { cubicOut } from "svelte/easing";
	import { crossfade, fade } from "svelte/transition";

	// Overview / Analytics switcher for a single recast. Mirrors SettingsTabs so
	// the two tab surfaces read identically across the dashboard.
	let { id }: { id: string } = $props();

	const tabs = $derived([
		{ label: "Overview", href: `/dashboard/recasts/${id}`, icon: Clapperboard },
		{ label: "Analytics", href: `/dashboard/recasts/${id}/analytics`, icon: BarChart3 },
	]);

	const path = $derived(page.url.pathname);

	// Slides the underline between tabs.
	const [send, receive] = crossfade({
		duration: 260,
		easing: cubicOut,
		fallback: (node) => fade(node, { duration: 120 }),
	});
</script>

<nav
	class="flex overflow-x-auto overflow-y-hidden border-b border-border-low/60"
	aria-label="Recast sections"
>
	{#each tabs as tab (tab.href)}
		{@const active = path === tab.href}
		{@const Icon = tab.icon}
		<a
			href={tab.href}
			aria-current={active ? "page" : undefined}
			class={cn(
				"group relative flex min-h-11 shrink-0 items-center gap-2 px-3.5 py-2.5 text-sm font-medium transition-colors duration-200",
				active ? "text-foreground" : "text-muted-foreground hover:text-foreground",
			)}
		>
			<Icon
				class="size-4 transition-colors {active
					? 'text-primary'
					: 'text-muted-foreground group-hover:text-foreground'}"
			/>
			{tab.label}
			{#if active}
				<span
					in:receive={{ key: "recast-tab" }}
					out:send={{ key: "recast-tab" }}
					class="absolute inset-x-2.5 -bottom-px h-0.5 rounded-full bg-primary"
					aria-hidden="true"
				></span>
			{/if}
		</a>
	{/each}
</nav>
