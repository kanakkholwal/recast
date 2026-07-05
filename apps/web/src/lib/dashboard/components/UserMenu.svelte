<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { authClient } from "$lib/auth/client";
	import { quotaStore, settingsStore } from "$lib/dashboard/store.svelte";
	import {
		ArrowUpRight,
		ChevronsUpDown,
		LayoutDashboard,
		LogOut,
		Settings,
		Shield,
		User,
	} from "@lucide/svelte";
	import * as DropdownMenu from "@recast/ui/dropdown-menu";

	const profile = $derived(settingsStore.value.profile);

	// Falls back to "user" if absent so the conditional safely returns false on
	// unauthenticated pages.
	const isAdmin = $derived(
		(page.data?.user as { role?: string } | undefined)?.role === "admin",
	);

	// Plan label under the name — prefers the active org's plan, then the quota
	// snapshot, else "free".
	const plan = $derived(
		((page.data as { activeOrganization?: { plan?: string } }).activeOrganization?.plan ??
			quotaStore.value?.plan ??
			"free") as string,
	);
	const planLabel = $derived(`${plan.charAt(0).toUpperCase()}${plan.slice(1)} plan`);

	async function signOut() {
		await authClient.signOut();
		await goto("/login");
	}
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class="flex items-center gap-2.5 rounded-lg py-1 pl-1 pr-1.5 text-left outline-none transition-colors hover:bg-foreground/5 focus-visible:ring-2 focus-visible:ring-ring/50"
		aria-label="Account menu"
	>
		<span class="grid size-8 shrink-0 place-items-center rounded-lg bg-linear-to-br from-primary/80 to-primary text-xs font-bold text-background">
			{settingsStore.initials}
		</span>
		<span class="hidden min-w-0 flex-col leading-tight sm:flex">
			<span class="truncate text-[12.5px] font-semibold text-foreground">
				{profile.name}
			</span>
			<span class="truncate text-[11px] text-muted-foreground">
				{planLabel}
			</span>
		</span>
		<ChevronsUpDown class="hidden size-3.5 shrink-0 text-muted-foreground sm:block" />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content side="bottom" align="end" sideOffset={8} class="w-56">
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
			<DropdownMenu.Item onclick={() => goto("/dashboard")}>
				<LayoutDashboard class="size-4 text-muted-foreground" />
				Dashboard
			</DropdownMenu.Item>
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
