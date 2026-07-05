<script lang="ts">
	import DashboardHeader from "$lib/dashboard/components/DashboardHeader.svelte";
	import DashboardSidebar from "$lib/dashboard/components/DashboardSidebar.svelte";
	import { settingsStore } from "$lib/dashboard/store.svelte";
	import {
		Building2,
		ClipboardList,
		CreditCard,
		Hourglass,
		LayoutDashboard,
		Users,
	} from "@lucide/svelte";
	import { NavProgress } from "@recast/ui/nav-progress";
	import * as Sidebar from "@recast/ui/sidebar";
	import { onMount } from "svelte";

	let { children, data } = $props();

	// Admin shares the dashboard's inset shell + sidebar, just with the admin
	// nav and no org switcher. The sidebar footer reads the profile from the
	// shared store, so hydrate it from this layout's load (which exposes
	// `data.admin`, not `data.user`).
	onMount(() => {
		settingsStore.value.profile.name = data.admin.name || data.admin.email;
		settingsStore.value.profile.email = data.admin.email;
	});

	const nav = [
		{ title: "Overview", href: "/admin", icon: LayoutDashboard, exact: true },
		{ title: "Users", href: "/admin/users", icon: Users, exact: false },
		{ title: "Teams", href: "/admin/teams", icon: Building2, exact: false },
		{ title: "Waitlist", href: "/admin/waitlist", icon: Hourglass, exact: false },
		{ title: "Subscriptions", href: "/admin/subscriptions", icon: CreditCard, exact: false },
		{ title: "Audit log", href: "/admin/audit", icon: ClipboardList, exact: false },
	];

	const labels: Record<string, string> = {
		users: "Users",
		teams: "Teams",
		waitlist: "Waitlist",
		subscriptions: "Subscriptions",
		audit: "Audit log",
	};
</script>

<svelte:head>
	<title>Admin - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<!-- Top-of-page navigation indicator, shared with the desktop app. -->
<NavProgress />

<!-- Same inset shell as /dashboard: `variant="inset"` turns the provider
	 wrapper `bg-sidebar` and floats the content as a rounded `bg-background`
	 panel. The impersonation indicator lives in the root layout so it floats
	 above every route, not just /admin. -->
<Sidebar.Provider>
	<DashboardSidebar
		{nav}
		subtitle="Admin"
		groupLabel="Administration"
		homeHref="/admin"
		showOrgSwitcher={false}
	/>
	<Sidebar.Inset>
		<DashboardHeader rootLabel="Overview" {labels} />
		<div class="px-5 py-8 sm:px-8 sm:py-10">
			{@render children()}
		</div>
	</Sidebar.Inset>
</Sidebar.Provider>
