<script lang="ts">
	import SettingsTabs from "$lib/dashboard/components/SettingsTabs.svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let { children } = $props();
</script>

<svelte:head>
	<title>Settings — Recast Dashboard</title>
</svelte:head>

<header in:fly={{ y: 12, duration: 500, easing: cubicOut }}>
	<h1 class="text-2xl font-semibold tracking-tight text-foreground">Settings</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		Manage your profile, integrations, and recording defaults.
	</p>
</header>

<div class="mt-6" in:fly={{ y: 10, duration: 460, delay: 80, easing: cubicOut }}>
	<SettingsTabs />
</div>

<div class="mt-6 max-w-2xl">
	{@render children()}
</div>
