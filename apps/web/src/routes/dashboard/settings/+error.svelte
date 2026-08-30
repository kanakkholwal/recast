<script lang="ts">
import { page } from "$app/state";
import SectionError from "$lib/components/SectionError.svelte";

// Finer-grained than the dashboard boundary: a tab's load error keeps the settings header and tab bar usable.
const status = $derived(page.status);
const message = $derived(page.error?.message ?? "");
</script>

<svelte:head>
	<title>{status} - Settings - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<SectionError {status} {message} homeHref="/dashboard/settings/profile" homeLabel="Back to settings" />
