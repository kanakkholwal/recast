<script lang="ts">
import { ArrowLeft, FileQuestion, Library } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { page } from "$app/state";
import SectionError from "$lib/components/SectionError.svelte";
import EmptyState from "$lib/dashboard/components/EmptyState.svelte";
import PageHeader from "$lib/dashboard/components/PageHeader.svelte";

// A failing `[id]` layout load surfaces here, so a missing or out-of-scope
// recast reads as "not found" instead of the generic dashboard boundary.
const status = $derived(page.status);
const notFound = $derived(status === 404);
</script>

<svelte:head>
	<title>{notFound ? "Recast not found" : status} - Recast Dashboard</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

{#if notFound}
	<PageHeader icon={Library} title="Recasts" subtitle="That link didn't lead anywhere." />
	<div class="surface mt-6">
		<EmptyState
			bordered={false}
			icon={FileQuestion}
			title="Recast not found"
			description="It was deleted, it lives in another workspace, or it isn't yours to open."
		>
			<Button href="/dashboard/recasts" size="sm" variant="dark" class="gap-2">
				<ArrowLeft class="size-3.5" />
				Back to library
			</Button>
		</EmptyState>
	</div>
{:else}
	<SectionError
		{status}
		message={page.error?.message ?? ""}
		homeHref="/dashboard/recasts"
		homeLabel="Back to library"
	/>
{/if}
