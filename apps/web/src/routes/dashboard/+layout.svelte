<script lang="ts">
	import DashboardHeader from "$lib/dashboard/components/DashboardHeader.svelte";
	import DashboardSidebar from "$lib/dashboard/components/DashboardSidebar.svelte";
	import { settingsStore } from "$lib/dashboard/store.svelte";
	import { NavProgress } from "@recast/ui/nav-progress";
	import * as Sidebar from "@recast/ui/sidebar";
	import { onMount } from "svelte";

	let { children, data } = $props();

	// Hydrate the dashboard's local store with the real signed-in user.
	onMount(() => {
		settingsStore.value.profile.name = data.user.name || data.user.email;
		settingsStore.value.profile.email = data.user.email;
	});
</script>

<!-- Top-of-page navigation indicator. Driven by SvelteKit's `navigating`
	 store inside the component; renders nothing when idle. -->
<NavProgress />

<Sidebar.Provider>
	<DashboardSidebar />
	<Sidebar.Inset class="min-h-svh">
		<DashboardHeader />
		<div class="px-5 py-8 sm:px-8 sm:py-10">
			{@render children()}
		</div>
	</Sidebar.Inset>
</Sidebar.Provider>
