<script lang="ts">
	import { page } from "$app/state";
	import SectionError from "$lib/components/SectionError.svelte";

	// Scoped boundary for the admin section: keeps the admin sidebar + header
	// on a page load error or streamed-promise rejection, rather than dropping
	// the operator onto the root full-page error.
	const status = $derived(page.status);
	const message = $derived(page.error?.message ?? "");
</script>

<svelte:head>
	<title>{status} - Admin - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<SectionError {status} {message} homeHref="/admin" homeLabel="Back to admin" />
