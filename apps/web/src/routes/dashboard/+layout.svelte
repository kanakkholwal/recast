<script lang="ts">
import DashboardHeader from "$lib/dashboard/components/DashboardHeader.svelte";
import DashboardSidebar from "$lib/dashboard/components/DashboardSidebar.svelte";
import QuickUploadDialog from "$lib/dashboard/components/QuickUploadDialog.svelte";
import { quotaStore, settingsStore } from "$lib/dashboard/store.svelte";
import { navigating } from "$app/state";
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

// Has this workspace ever published a recast (active or archived)? Drives the
// upload dialog's endowed-progress framing for genuine first-timers.
const firstUpload = $derived(
	(data.quota?.usage.activeRecastsCount ?? 0) + (data.quota?.usage.archivedRecastsCount ?? 0) === 0,
);
</script>
<svelte:head>
	<title>Dashboard - Recast</title>
</svelte:head>
<NavProgress active={navigating.to !== null} />
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
	{firstUpload}
/>
