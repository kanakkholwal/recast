<script lang="ts">
	import favicon from "$lib/assets/favicon.svg";
	import { Toaster } from "@recast/ui/sonner";
	import { ModeWatcher } from "@recast/ui/theme";
	import "../app.css";
	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" type="image/svg+xml" href={favicon} />
</svelte:head>

<ModeWatcher />
<Toaster position="top-center" duration={5000}/>

<div class="relative flex min-h-screen flex-col overflow-x-hidden">
	{@render children()}
</div>

<style>
	:global(html) {
		scroll-behavior: smooth;
	}
</style>
