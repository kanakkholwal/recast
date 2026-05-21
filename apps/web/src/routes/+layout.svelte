<script lang="ts">
	import { dev } from "$app/environment";
	import { page } from "$app/state";
	import { DevThemeToggle, Navbar } from "$lib/components";
	import { Toaster } from "@recast/ui/sonner";
	import { ModeWatcher } from "@recast/ui/theme";
	import "../app.css";

	let { children } = $props();

	// The dashboard, auth, and waitlist screens ship their own focused
	// shells — keep the marketing chrome off them.
	const chromelessPaths = new Set([
		"/login",
		"/signup",
		"/forgot-password",
		"/reset-password",
		"/waitlist",
	]);
	const isChromeless = $derived(
		page.url.pathname.startsWith("/dashboard") ||
			page.url.pathname.startsWith("/admin") ||
			chromelessPaths.has(page.url.pathname),
	);
</script>

<ModeWatcher />

{#if !isChromeless}
	<div
		aria-hidden="true"
		class="bg-grid bg-grid-fade pointer-events-none fixed inset-0 -z-10 opacity-30"
	></div>

	<Navbar />
{/if}

<div class="relative isolate flex min-h-screen flex-col overflow-x-hidden">
	{@render children()}
</div>

<Toaster position="bottom-right" duration={5000} />

{#if dev}
	<DevThemeToggle />
{/if}
