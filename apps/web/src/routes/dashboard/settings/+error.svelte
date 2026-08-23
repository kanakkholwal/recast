<script lang="ts">
import { page } from "$app/state";
import SectionError from "$lib/components/SectionError.svelte";

// Finer-grained boundary than dashboard/+error: a settings *tab* load error
// renders here, inside the settings layout — so the settings header + tab bar
// stay put and the operator can switch tabs without a full reload.
const status = $derived(page.status);
const message = $derived(page.error?.message ?? "");
</script>

<svelte:head>
	<title>{status} - Settings - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<SectionError {status} {message} homeHref="/dashboard/settings/profile" homeLabel="Back to settings" />
