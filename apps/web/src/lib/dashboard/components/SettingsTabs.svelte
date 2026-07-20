<script lang="ts">
	import { page } from "$app/state";
	import { CreditCard, Settings2, User } from "@recast/icons";
	import { cn } from "@recast/ui/utils";
	import { cubicOut } from "svelte/easing";
	import { crossfade, fade } from "svelte/transition";

	const tabs = [
		{ label: "Profile", href: "/dashboard/settings/profile", icon: User },
		{ label: "Preferences", href: "/dashboard/settings/preferences", icon: Settings2 },
		{ label: "Plan & billing", href: "/dashboard/settings/billing", icon: CreditCard },
	];

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
	aria-label="Settings sections"
>
	{#each tabs as tab (tab.href)}
		{@const active = path === tab.href}
		{@const Icon = tab.icon}
		<a
			href={tab.href}
			aria-current={active ? "page" : undefined}
			class={cn(
				"group relative flex min-h-11 shrink-0 items-center gap-2 px-3.5 py-2.5 text-sm font-medium transition-colors duration-200",
				active
					? "text-foreground"
					: "text-muted-foreground hover:text-foreground",
			)}
		>
			<Icon
				class="size-4 transition-colors {active
					? 'text-foreground'
					: 'text-muted-foreground group-hover:text-foreground'}"
			/>
			{tab.label}
			{#if active}
				<span
					in:receive={{ key: "settings-tab" }}
					out:send={{ key: "settings-tab" }}
					class="absolute inset-x-2.5 -bottom-px h-0.5 rounded-full bg-foreground"
					aria-hidden="true"
				></span>
			{/if}
		</a>
	{/each}
</nav>
