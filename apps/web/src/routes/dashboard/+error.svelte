<script lang="ts">
import { page } from "$app/state";
import SectionError from "$lib/components/SectionError.svelte";

// Scoped boundary: a page load error renders inside the dashboard shell rather than losing the sidebar and header.
const status = $derived(page.status);
const message = $derived(page.error?.message ?? "");
</script>

<svelte:head>
	<title>{status} - Dashboard - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<SectionError {status} {message} homeHref="/dashboard" homeLabel="Back to dashboard" />
