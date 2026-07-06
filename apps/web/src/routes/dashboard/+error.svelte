<script lang="ts">
	import { page } from "$app/state";
	import SectionError from "$lib/components/SectionError.svelte";

	// Scoped boundary: a dashboard page's load error (or a streamed-promise
	// rejection) renders here — inside the dashboard shell — instead of
	// escalating to the root full-page error and losing the sidebar + header.
	const status = $derived(page.status);
	const message = $derived(page.error?.message ?? "");
</script>

<svelte:head>
	<title>{status} - Dashboard - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<SectionError {status} {message} homeHref="/dashboard" homeLabel="Back to dashboard" />
