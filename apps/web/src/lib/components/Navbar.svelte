<script lang="ts">
import { LayoutDashboard, Menu } from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import { page } from "$app/state";
import { authClient } from "$lib/auth/client";
import { GITHUB_URL, menuGroups, navLinks } from "$lib/components/nav-data";
import Logo from "$lib/logo.svelte";
import MegaMenu from "./MegaMenu.svelte";
import MobileNav from "./MobileNav.svelte";

let open = $state(false);
let scrolled = $state(false);

const pathname = $derived(page.url.pathname);
const session = authClient.useSession();
const signedIn = $derived(Boolean($session.data?.user));

// Navigating from inside the sheet should leave it closed.
$effect(() => {
	void pathname;
	open = false;
});

const linkClass =
	"inline-flex items-center whitespace-nowrap rounded-full px-3.5 py-2 text-body-sm font-medium transition-colors hover:text-foreground motion-reduce:transition-none";

const isCurrent = (href: string) => pathname === href || pathname.startsWith(`${href}/`);
</script>

<svelte:window onscroll={() => (scrolled = window.scrollY > 8)} />

<div
	class={cn(
		"fixed inset-x-0 top-0 z-50 border-b transition-colors duration-200 motion-reduce:transition-none",
		scrolled ? "border-border-low bg-background/85 backdrop-blur" : "border-transparent",
	)}
>
	<nav
		aria-label="Primary"
		class="mx-auto flex h-16 w-full max-w-6xl items-center gap-2 px-6 sm:px-8 lg:px-10"
	>
		<a
			href="/"
			class="flex shrink-0 items-center gap-2.5 rounded-lg py-1 pr-2"
			aria-label="Recast home"
		>
			<span class="grid size-7 place-items-center rounded-lg bg-foreground p-1 text-background">
				<Logo size="20" color="transparent" fill="currentColor" />
			</span>
			<span class="whitespace-nowrap font-display text-lg font-semibold tracking-wide text-foreground">
				Recast
			</span>
		</a>

		<div class="hidden flex-1 items-center justify-center md:flex">
			<MegaMenu groups={menuGroups} {pathname} />
			<ul class="flex items-center gap-1">
				{#each navLinks as link (link.href)}
					<li>
						<a
							href={link.href}
							aria-current={isCurrent(link.href) ? "page" : undefined}
							class={cn(linkClass, isCurrent(link.href) ? "text-foreground" : "text-muted-foreground")}
						>
							{link.label}
						</a>
					</li>
				{/each}
			</ul>
		</div>

		<div class="ml-auto flex shrink-0 items-center gap-2 md:ml-0">
			<a
				href={GITHUB_URL}
				target="_blank"
				rel="noopener noreferrer"
				aria-label="Recast on GitHub"
				class="hidden size-9 place-items-center rounded-lg text-muted-foreground transition-colors hover:text-foreground md:grid motion-reduce:transition-none"
			>
				<GithubBrand class="size-4" />
			</a>
			{#if signedIn}
				<a href="/dashboard" class={cn("hidden text-muted-foreground md:inline-flex", linkClass)}>
					<LayoutDashboard class="mr-1.5 size-3.5" />
					Dashboard
				</a>
			{:else}
				<a href="/login" class={cn("hidden text-muted-foreground md:inline-flex", linkClass)}>
					Sign in
				</a>
			{/if}
			<Button href="/download" size="sm" variant="dark">Download</Button>
			<button
				type="button"
				onclick={() => (open = true)}
				aria-expanded={open}
				aria-label="Open menu"
				class="grid size-9 cursor-pointer place-items-center rounded-lg text-foreground transition-colors hover:bg-paper md:hidden motion-reduce:transition-none"
			>
				<Menu class="size-5" />
			</button>
		</div>
	</nav>
</div>

<MobileNav bind:open groups={menuGroups} links={navLinks} {signedIn} {pathname} />
