<script lang="ts">
import { ArrowLeft, ImagePlus, Loader2, Pencil, Share2 } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import type { Snippet } from "svelte";
import { invalidateAll } from "$app/navigation";
import EditRecastDetailsDialog from "$lib/dashboard/components/EditRecastDetailsDialog.svelte";
import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
import RecastTabs from "$lib/dashboard/components/RecastTabs.svelte";
import RecastShareDialog from "$lib/dashboard/components/ShareRecastDialog.svelte";
import { POSTER_ACCEPT, replacePoster } from "$lib/dashboard/poster";
import { formatRecastSubtitle } from "$lib/dashboard/recast-detail.logic";
import { shareDialog } from "$lib/dashboard/share-dialog.svelte";
import type { LayoutData } from "./$types";

let { data, children }: { data: LayoutData; children: Snippet } = $props();

const recast = $derived(data.recast);
const subtitle = $derived(formatRecastSubtitle(recast));

let editOpen = $state(false);

// ── Replace cover ───────────────────────────────────────────────────
let posterInput = $state<HTMLInputElement | null>(null);
let replacingPoster = $state(false);

function pickPoster() {
	if (replacingPoster) return;
	posterInput?.click();
}

async function onPosterPick(e: Event) {
	const input = e.currentTarget as HTMLInputElement;
	const file = input.files?.[0];
	input.value = ""; // allow re-picking the same file later
	if (!file) return;
	replacingPoster = true;
	try {
		await replacePoster(recast.id, file);
		// Re-run the loader so the (re-signed) poster URL flows back in.
		await invalidateAll();
		toast.success("Cover updated.");
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't update the cover.");
	} finally {
		replacingPoster = false;
	}
}
</script>

<a
	href="/dashboard/recasts"
	class="group/back mb-4 inline-flex items-center gap-1.5 text-body-sm font-medium text-muted-foreground transition-colors hover:text-foreground motion-reduce:transition-none"
>
	<ArrowLeft
		class="size-3.5 transition-transform group-hover/back:-translate-x-0.5 motion-reduce:transition-none"
	/>
	Library
</a>

<PageHeader title={recast.title} {subtitle}>
	<Button variant="outline" size="sm" class="gap-2" onclick={() => (editOpen = true)}>
		<Pencil class="size-3.5" />
		Edit details
	</Button>
	<Button variant="outline" size="sm" class="gap-2" disabled={replacingPoster} onclick={pickPoster}>
		{#if replacingPoster}
			<Loader2 class="size-3.5 animate-spin" />
			Saving…
		{:else}
			<ImagePlus class="size-3.5" />
			Change cover
		{/if}
	</Button>
	<Button size="sm" variant="dark" class="gap-2" onclick={() => shareDialog.show(recast.id)}>
		<Share2 class="size-3.5" />
		Share
	</Button>
</PageHeader>

<input
	bind:this={posterInput}
	type="file"
	accept={POSTER_ACCEPT}
	class="hidden"
	onchange={onPosterPick}
/>

<div class="mt-5">
	<RecastTabs id={recast.id} />
</div>

<div class="mt-6">
	{@render children()}
</div>

<RecastShareDialog />

<EditRecastDetailsDialog
	bind:open={editOpen}
	recastId={recast.id}
	title={recast.title}
	description={recast.description}
/>
