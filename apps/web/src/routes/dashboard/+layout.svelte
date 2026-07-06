<script lang="ts">
	import DashboardHeader from "$lib/dashboard/components/DashboardHeader.svelte";
	import DashboardSidebar from "$lib/dashboard/components/DashboardSidebar.svelte";
	import QuickUploadDialog from "$lib/dashboard/components/QuickUploadDialog.svelte";
	import { quotaStore, settingsStore } from "$lib/dashboard/store.svelte";
	import { NavProgress } from "@recast/ui/nav-progress";
	import * as Sidebar from "@recast/ui/sidebar";
	import { onMount } from "svelte";

	let { children, data } = $props();

	// Hydrate the dashboard's local store with the real signed-in user.
	onMount(() => {
		settingsStore.value.profile.name = data.user.name || data.user.email;
		settingsStore.value.profile.email = data.user.email;
	});

	// Reactive re-hydration of quota — re-runs when the loader returns a
	// new snapshot (e.g. after `invalidateAll()` post-upload).
	$effect(() => {
		quotaStore.hydrate(data.quota ?? null);
	});
</script>
<svelte:head>
	<title>Dashboard - Recast</title>
</svelte:head>
<NavProgress />
<Sidebar.Provider>
	<DashboardSidebar />
	<Sidebar.Inset>
		<DashboardHeader />
		<div class="px-5 py-8 sm:px-8 sm:py-10">
			{@render children()}
		</div>
	</Sidebar.Inset>
</Sidebar.Provider>

<QuickUploadDialog
	workspaceId={data.activeOrganization?.id}
	workspaceName={data.activeOrganization?.name}
	plan={data.activeOrganization?.plan}
/>
