<script lang="ts">
	import { dev } from "$app/environment";
	import { DevThemeToggle, Navbar } from "$lib/components";
	import { Toaster } from "@recast/ui/sonner";
	import { ModeWatcher } from "@recast/ui/theme";
	import "../app.css";

	let { children } = $props();
</script>

<ModeWatcher />

<div
	aria-hidden="true"
	class="pointer-events-none fixed inset-0 -z-10 bg-ambient"
></div>
<div
	aria-hidden="true"
	class="bg-grid bg-grid-fade pointer-events-none fixed inset-0 -z-10 opacity-30"
></div>

<Navbar />

<div class="relative isolate flex min-h-screen flex-col overflow-x-hidden">
	{@render children()}
</div>

<Toaster position="bottom-right" duration={5000} />

{#if dev}
	<DevThemeToggle />
{/if}
