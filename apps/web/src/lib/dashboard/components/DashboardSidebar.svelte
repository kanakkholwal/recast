<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { authClient } from "$lib/auth/client";
	import { settingsStore } from "$lib/dashboard/store.svelte";
	import Logo from "$lib/logo.svelte";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";
	import * as Sidebar from "@recast/ui/sidebar";
	import { useSidebar } from "@recast/ui/sidebar";
	import { mode, toggleMode } from "@recast/ui/theme";
	import { cn } from "@recast/ui/utils";
	import {
		ArrowUpRight,
		BarChart3,
		ChevronsUpDown,
		Film,
		LayoutDashboard,
		LogOut,
		Moon,
		Settings,
		Shield,
		Sun,
		User,
	} from "@lucide/svelte";
	import type { ComponentProps } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { crossfade, fade, fly } from "svelte/transition";

	const sidebar = useSidebar();
	const open = $derived(sidebar.state === "expanded");
	const currentPath = $derived(page.url.pathname);
	const profile = $derived(settingsStore.value.profile);
	// Surfaced by /dashboard/+layout.server.ts; falls back to "user" if absent
	// so the conditional below safely returns false on unauthenticated pages.
	const isAdmin = $derived(
		(page.data?.user as { role?: string } | undefined)?.role === "admin",
	);

	const nav = [
		{ title: "Home", href: "/dashboard", icon: LayoutDashboard, exact: true },
		{ title: "Recasts", href: "/dashboard/recasts", icon: Film, exact: false },
		{ title: "Analytics", href: "/dashboard/analytics", icon: BarChart3, exact: false },
		{ title: "Settings", href: "/dashboard/settings", icon: Settings, exact: false },
	];

	function isActive(href: string, exact: boolean) {
		return exact ? currentPath === href : currentPath.startsWith(href);
	}

	// Slides the active highlight between rows rather than cross-fading in place.
	const [send, receive] = crossfade({
		duration: 280,
		easing: cubicOut,
		fallback: (node) => fade(node, { duration: 120 }),
	});

	async function signOut() {
		await authClient.signOut();
		await goto("/login");
	}
</script>

<Sidebar.Root variant="floating" collapsible="icon">
	<Sidebar.Rail />

	<Sidebar.Header class="gap-3 py-3">
		<a
			href="/dashboard"
			aria-label="Recast dashboard"
			class={cn(
				"flex h-10 items-center gap-2.5 overflow-hidden rounded-lg transition-opacity hover:opacity-80",
				open ? "px-1.5" : "justify-center px-0",
			)}
		>
			<span class="grid size-8 shrink-0 place-items-center rounded-lg bg-foreground p-1 text-background shadow-craft-sm">
				<Logo size="20" color="transparent" fill="currentColor" />
			</span>
			{#if open}
				<span
					in:fly={{ x: -8, duration: 240, easing: cubicOut, delay: 60 }}
					out:fade={{ duration: 180, easing: cubicOut }}
					class="flex flex-col leading-none"
				>
					<span class="text-[15px] font-semibold tracking-tight text-foreground">
						Recast
					</span>
					<span class="mt-0.5 text-[10px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
						Dashboard
					</span>
				</span>
			{/if}
		</a>
	</Sidebar.Header>

	<Sidebar.Content class="scrollbar-hide">
		<Sidebar.Group>
			{#if open}
				<Sidebar.GroupLabel
					class="px-2 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/70"
				>
					<span
						in:fade={{ duration: 180, delay: 80, easing: cubicOut }}
						out:fade={{ duration: 140, easing: cubicOut }}
					>
						Library
					</span>
				</Sidebar.GroupLabel>
			{/if}
			<Sidebar.GroupContent>
				<Sidebar.Menu class="gap-0.5">
					{#each nav as link (link.href)}
						{@const active = isActive(link.href, link.exact)}
						{@const Icon = link.icon}
						<Sidebar.MenuItem>
							<Sidebar.MenuButton tooltipContent={link.title}>
								{#snippet child({
									props,
								}: {
									props: ComponentProps<typeof Sidebar.MenuButton>;
								})}
									<a
										href={link.href}
										{...props as Record<string, unknown>}
										data-active={active}
										class={cn(
											"group/item relative flex h-9 items-center gap-2.5 overflow-hidden rounded-lg text-[12.5px] font-medium transition-colors duration-200",
											active
												? "text-foreground"
												: "text-muted-foreground hover:text-foreground",
											open ? "px-2.5" : "size-8 justify-center p-0",
										)}
									>
										{#if active}
											<span
												in:receive={{ key: "nav-active-bg" }}
												out:send={{ key: "nav-active-bg" }}
												class="absolute inset-0 z-0 rounded-lg bg-foreground/6 ring-1 ring-inset ring-border/40"
												aria-hidden="true"
											></span>
											{#if open}
												<span
													in:receive={{ key: "nav-active-pill" }}
													out:send={{ key: "nav-active-pill" }}
													class="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full bg-primary"
													aria-hidden="true"
												></span>
											{/if}
										{/if}
										<Icon
											size={14}
											class="relative z-10 shrink-0 transition-transform duration-200 group-hover/item:-translate-y-px group-active/item:scale-95"
										/>
										{#if open}
											<span
												in:fly={{ x: -6, duration: 220, easing: cubicOut, delay: 40 }}
												out:fade={{ duration: 160, easing: cubicOut }}
												class="relative z-10 truncate"
											>
												{link.title}
											</span>
										{/if}
									</a>
								{/snippet}
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
					{/each}
				</Sidebar.Menu>
			</Sidebar.GroupContent>
		</Sidebar.Group>
	</Sidebar.Content>

	<Sidebar.Footer class="gap-1 border-t border-border/30 p-2">
		<button
			type="button"
			onclick={toggleMode}
			aria-label={mode.current === "dark" ? "Switch to light mode" : "Switch to dark mode"}
			title={mode.current === "dark" ? "Light mode" : "Dark mode"}
			class={cn(
				"group/theme relative flex h-9 items-center gap-2.5 overflow-hidden rounded-lg text-[12.5px] font-medium text-muted-foreground transition-colors duration-200 hover:bg-foreground/5 hover:text-foreground",
				open ? "px-2.5" : "size-8 justify-center self-center p-0",
			)}
		>
			<span class="relative grid size-3.5 place-items-center">
				{#if mode.current === "dark"}
					<span
						class="absolute grid place-items-center transition-transform duration-300 group-hover/theme:rotate-45"
						in:fly={{ y: 4, duration: 180, easing: cubicOut }}
						out:fade={{ duration: 120 }}
					>
						<Sun class="size-3.5" />
					</span>
				{:else}
					<span
						class="absolute grid place-items-center transition-transform duration-300 group-hover/theme:-rotate-12"
						in:fly={{ y: -4, duration: 180, easing: cubicOut }}
						out:fade={{ duration: 120 }}
					>
						<Moon class="size-3.5" />
					</span>
				{/if}
			</span>
			{#if open}
				<span
					in:fly={{ x: -6, duration: 220, easing: cubicOut, delay: 40 }}
					out:fade={{ duration: 160, easing: cubicOut }}
					class="truncate"
				>
					{mode.current === "dark" ? "Light mode" : "Dark mode"}
				</span>
			{/if}
		</button>

		<div class="my-1 h-px bg-border/30"></div>

		<DropdownMenu.Root>
			<DropdownMenu.Trigger
				class={cn(
					"flex w-full items-center gap-2.5 rounded-lg p-1.5 text-left outline-none transition-colors hover:bg-foreground/5 focus-visible:ring-2 focus-visible:ring-ring/50",
					!open && "justify-center",
				)}
			>
				<span class="grid size-8 shrink-0 place-items-center rounded-lg bg-linear-to-br from-primary/80 to-primary text-xs font-bold text-background">
					{settingsStore.initials}
				</span>
				{#if open}
					<span class="flex min-w-0 flex-1 flex-col" in:fade={{ duration: 160 }}>
						<span class="truncate text-[12.5px] font-semibold text-foreground">
							{profile.name}
						</span>
						<span class="truncate text-[11px] text-muted-foreground">
							{profile.email}
						</span>
					</span>
					<ChevronsUpDown class="size-3.5 shrink-0 text-muted-foreground" />
				{/if}
			</DropdownMenu.Trigger>
			<DropdownMenu.Content side="top" align="start" sideOffset={8} class="w-56">
				<DropdownMenu.Label>
					<span class="block truncate text-sm font-semibold text-foreground">
						{profile.name}
					</span>
					<span class="block truncate text-xs font-normal text-muted-foreground">
						{profile.email}
					</span>
				</DropdownMenu.Label>
				<DropdownMenu.Separator />
				<DropdownMenu.Item onclick={() => goto("/dashboard/settings/profile")}>
					<User class="size-4 text-muted-foreground" />
					Profile
				</DropdownMenu.Item>
				<DropdownMenu.Item onclick={() => goto("/dashboard/settings")}>
					<Settings class="size-4 text-muted-foreground" />
					Settings
				</DropdownMenu.Item>
				{#if isAdmin}
					<DropdownMenu.Item onclick={() => goto("/admin")}>
						<Shield class="size-4 text-primary" />
						Admin dashboard
					</DropdownMenu.Item>
				{/if}
				<DropdownMenu.Item onclick={() => goto("/")}>
					<ArrowUpRight class="size-4 text-muted-foreground" />
					Back to site
				</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item
					onclick={signOut}
					class="text-destructive/90 data-highlighted:text-destructive"
				>
					<LogOut class="size-4" />
					Sign out
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	</Sidebar.Footer>
</Sidebar.Root>

<style>
	.scrollbar-hide {
		-ms-overflow-style: none;
		scrollbar-width: none;
	}
	.scrollbar-hide::-webkit-scrollbar {
		display: none;
	}
</style>
