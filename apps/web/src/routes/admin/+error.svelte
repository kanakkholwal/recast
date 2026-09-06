<script lang="ts">
import { page } from "$app/state";
import SectionError from "$lib/components/SectionError.svelte";

// Scoped to admin: a load error keeps the sidebar and header instead of dropping the operator on the root error page.
const status = $derived(page.status);
const message = $derived(page.error?.message ?? "");
</script>

<svelte:head>
	<title>{status} - Admin - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<SectionError {status} {message} homeHref="/admin" homeLabel="Back to admin" />
