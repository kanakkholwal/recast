<script lang="ts">
	import { page } from "$app/state";
	import { cn } from "@recast/ui/utils";
	import {
		ArrowLeft,
		Building2,
		ClipboardList,
		Crown,
		CreditCard,
		Hourglass,
		LayoutDashboard,
		Users,
	} from "@lucide/svelte";

	let { children, data } = $props();

	const currentPath = $derived(page.url.pathname);

	const nav = [
		{ href: "/admin", label: "Overview", icon: LayoutDashboard, exact: true },
		{ href: "/admin/users", label: "Users", icon: Users, exact: false },
		{ href: "/admin/teams", label: "Teams", icon: Building2, exact: false },
		{ href: "/admin/waitlist", label: "Waitlist", icon: Hourglass, exact: false },
		{ href: "/admin/subscriptions", label: "Subscriptions", icon: CreditCard, exact: false },
		{ href: "/admin/audit", label: "Audit log", icon: ClipboardList, exact: false },
	];

	function isActive(href: string, exact: boolean) {
		return exact ? currentPath === href : currentPath.startsWith(href);
	}
</script>

<svelte:head>
	<title>Admin — Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<div class="min-h-screen bg-background text-foreground">
	<!-- Impersonation indicator lives in the root layout (ImpersonationBanner)
	     so it floats above every route, not just /admin. -->

	<div class="mx-auto flex w-full max-w-7xl gap-8 px-5 py-8 sm:px-8 sm:py-10">
		<aside class="hidden w-56 shrink-0 lg:block">
			<div class="sticky top-8 flex flex-col gap-1">
				<a
					href="/dashboard"
					class="mb-4 inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground transition-colors hover:text-foreground"
				>
					<ArrowLeft class="size-3" />
					Back to dashboard
				</a>
				<div class="mb-3 flex items-center gap-2 px-2.5 py-2">
					<span
						class="grid size-7 place-items-center rounded-md bg-primary/15 text-primary"
					>
						<Crown class="size-3.5" />
					</span>
					<div class="flex flex-col leading-tight">
						<span class="text-sm font-semibold tracking-tight">Admin</span>
						<span class="text-[11px] text-muted-foreground">{data.admin.email}</span>
					</div>
				</div>
				{#each nav as link (link.href)}
					{@const active = isActive(link.href, link.exact)}
					{@const Icon = link.icon}
					<a
						href={link.href}
						class={cn(
							"flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] font-medium transition-colors",
							active
								? "bg-foreground/5 text-foreground ring-1 ring-inset ring-border/40"
								: "text-muted-foreground hover:bg-foreground/3 hover:text-foreground",
						)}
					>
						<Icon class="size-4" />
						{link.label}
					</a>
				{/each}
			</div>
		</aside>

		<main class="min-w-0 flex-1">
			{@render children()}
		</main>
	</div>
</div>
