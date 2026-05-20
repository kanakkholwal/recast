<script lang="ts">
	import DashboardHeader from "$lib/dashboard/components/DashboardHeader.svelte";
	import DashboardSidebar from "$lib/dashboard/components/DashboardSidebar.svelte";
	import * as Sidebar from "@recast/ui/sidebar";

	let { children } = $props();
</script>

<Sidebar.Provider>
	<DashboardSidebar />
	<Sidebar.Inset class="min-h-svh">
		<DashboardHeader />
		<div class="px-5 py-7 sm:px-8 sm:py-9">
			{@render children()}
		</div>
	</Sidebar.Inset>
</Sidebar.Provider>
