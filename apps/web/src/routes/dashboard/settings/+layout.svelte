<script lang="ts">
import { Settings } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
import SettingsTabs from "$lib/dashboard/components/SettingsTabs.svelte";

let { data, children } = $props();

const planLabel = $derived(
	`${data.activeOrganization.plan.charAt(0).toUpperCase()}${data.activeOrganization.plan.slice(1)} plan`,
);
</script>

<svelte:head>
	<title>Settings - Recast Dashboard</title>
</svelte:head>

<PageHeader
	icon={Settings}
	title="Settings"
	subtitle="Manage account details, workspace defaults, and billing for {data.activeOrganization.name}."
>
	<Badge variant="outline">{planLabel}</Badge>
</PageHeader>

<div class="mt-6" in:fly={{ y: 10, duration: 460, delay: 80, easing: cubicOut }}>
	<SettingsTabs />
</div>

<div class="mt-6">
	{@render children()}
</div>
