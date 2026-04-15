<script lang="ts">
	import favicon from "$lib/assets/favicon.svg";
	import { ModeWatcher } from "mode-watcher";
	import { Toaster } from "svelte-sonner";
	import "../app.css";
	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" type="image/svg+xml" href={favicon} />
</svelte:head>

<ModeWatcher />
<Toaster position="top-center" />

<div class="relative flex min-h-screen flex-col overflow-x-hidden">
	{@render children()}
</div>

<style>
	:global(html) {
		scroll-behavior: smooth;
	}
</style>
