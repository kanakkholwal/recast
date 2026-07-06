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

<!-- Top-of-page navigation indicator. Driven by SvelteKit's `navigating`
	 store inside the component; renders nothing when idle. -->
<NavProgress />

<!-- Inset shell, matching the desktop `(app)` group: `variant="inset"` turns the
	 provider wrapper `bg-sidebar` and floats the content as a rounded `bg-background`
	 panel with a gap. Unlike desktop (a fixed-viewport app), the web dashboard is a
	 scrolling document, so the panel grows with content and the page scrolls — the
	 header rounds/offsets its own top to sit cleanly inside the panel. -->
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
