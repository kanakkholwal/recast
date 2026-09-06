<script lang="ts">
import {
	ChevronsUpDown,
	Command,
	CreditCard,
	Crown,
	Home,
	LayoutDashboard,
	LogOut,
	Megaphone,
	Moon,
	Settings,
	Shield,
	Sun,
} from "@recast/icons";
import * as DropdownMenu from "@recast/ui/dropdown-menu";
import { Kbd } from "@recast/ui/kbd";
import { mode, toggleMode } from "@recast/ui/theme";
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { authClient } from "$lib/auth/client";
import { commandPalette } from "$lib/dashboard/command-palette.svelte";
import { quotaStore, settingsStore } from "$lib/dashboard/store.svelte";

const profile = $derived(settingsStore.value.profile);

// Falls back to 'user' when absent, so the check safely returns false on unauthenticated pages.
const isAdmin = $derived((page.data?.user as { role?: string } | undefined)?.role === "admin");
// Whether we're currently inside the admin shell — decides "enter" vs "exit".
const inAdmin = $derived(page.url.pathname.startsWith("/admin"));

// Prefers the active org's plan, then the quota snapshot, else free.
const plan = $derived(
	((page.data as { activeOrganization?: { plan?: string } }).activeOrganization?.plan ??
		quotaStore.value?.plan ??
		"free") as string,
);
const planLabel = $derived(`${plan.charAt(0).toUpperCase()}${plan.slice(1)} plan`);
const isFree = $derived(plan === "free");

async function signOut() {
	await authClient.signOut();
	await goto("/login");
}
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class="flex items-center gap-2.5 rounded-lg py-1 pl-1 pr-1.5 text-left outline-none transition-colors hover:bg-paper focus-visible:ring-2 focus-visible:ring-ring/50"
		aria-label="Account menu"
	>
		<span class="grid size-8 shrink-0 place-items-center rounded-lg bg-paper text-caption font-medium text-foreground">
			{settingsStore.initials}
		</span>
		<span class="hidden min-w-0 flex-col leading-tight sm:flex">
			<span class="truncate text-body-sm font-medium text-foreground">
				{profile.name}
			</span>
			<span class="truncate text-caption text-muted-foreground">
				{planLabel}
			</span>
		</span>
		<ChevronsUpDown class="hidden size-3.5 shrink-0 text-muted-foreground sm:block" />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content side="bottom" align="end" sideOffset={8} class="w-56">
		<DropdownMenu.Label>
			<span class="block truncate text-body-sm font-medium text-foreground">
				{profile.name}
			</span>
			<span class="block truncate text-caption font-normal text-muted-foreground">
				{profile.email}
			</span>
		</DropdownMenu.Label>
		<DropdownMenu.Separator />
		<DropdownMenu.Item onclick={() => goto("/dashboard/settings")}>
			<Settings class="size-4 text-muted-foreground" />
			Settings
		</DropdownMenu.Item>
		<DropdownMenu.Item onclick={() => goto("/dashboard/settings/billing")}>
			{#if isFree}
				<Crown class="size-4 text-primary" />
				<span class="text-foreground">Upgrade plan</span>
			{:else}
				<CreditCard class="size-4 text-muted-foreground" />
				Plan &amp; billing
			{/if}
		</DropdownMenu.Item>

		<DropdownMenu.Separator />
		<DropdownMenu.Item closeOnSelect={false} onclick={toggleMode}>
			{#if mode.current === "dark"}
				<Sun class="size-4 text-muted-foreground" />
				Light mode
			{:else}
				<Moon class="size-4 text-muted-foreground" />
				Dark mode
			{/if}
		</DropdownMenu.Item>
		<DropdownMenu.Item onclick={() => commandPalette.show()}>
			<Command class="size-4 text-muted-foreground" />
			Command menu
			<Kbd class="ml-auto">
				<span class="text-caption">⌘</span>
				<span class="text-caption">K</span>
			</Kbd>
		</DropdownMenu.Item>
		<DropdownMenu.Item onclick={() => goto("/changelog")}>
			<Megaphone class="size-4 text-muted-foreground" />
			What's new
		</DropdownMenu.Item>

		{#if isAdmin}
			<DropdownMenu.Separator />
			{#if inAdmin}
				<DropdownMenu.Item onclick={() => goto("/dashboard")}>
					<LayoutDashboard class="size-4 text-muted-foreground" />
					Exit admin
				</DropdownMenu.Item>
			{:else}
				<DropdownMenu.Item onclick={() => goto("/admin")}>
					<Shield class="size-4 text-muted-foreground" />
					Admin dashboard
				</DropdownMenu.Item>
			{/if}
		{/if}

		<DropdownMenu.Separator />
		<DropdownMenu.Item onclick={() => goto("/")}>
			<Home class="size-4 text-muted-foreground" />
			Back to site
		</DropdownMenu.Item>
		<DropdownMenu.Item
			onclick={signOut}
			class="text-destructive data-highlighted:text-destructive"
		>
			<LogOut class="size-4" />
			Sign out
		</DropdownMenu.Item>
	</DropdownMenu.Content>
</DropdownMenu.Root>
