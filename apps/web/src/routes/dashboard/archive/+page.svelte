<script lang="ts">
import { Archive } from "@recast/icons";
import { toast } from "@recast/ui/sonner";
import { flip } from "svelte/animate";
import { cubicOut } from "svelte/easing";
import { fly, scale } from "svelte/transition";
import * as api from "$lib/dashboard/api";
import ArchivedCard, { type ArchivedRecast } from "$lib/dashboard/components/ArchivedCard.svelte";
import ConfirmDialog from "$lib/dashboard/components/ConfirmDialog.svelte";
import EmptyState from "$lib/dashboard/components/EmptyState.svelte";
import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
import { formatBytes } from "$lib/dashboard/format";

let { data } = $props();
let archived = $state<ArchivedRecast[]>([]);
// Permanent, and the last copy of the metadata: worth one confirmation.
let confirmDelete = $state<ArchivedRecast | null>(null);
let deleting = $state(false);
$effect(() => {
	archived = data.archived;
});

const totalBytes = $derived(archived.reduce((sum, a) => sum + a.sizeBytes, 0));

async function deleteArchived(rec: ArchivedRecast) {
	if (deleting) return;
	deleting = true;
	const snapshot = archived;
	archived = archived.filter((a) => a.id !== rec.id);
	try {
		await api.deleteRecast(rec.id);
		confirmDelete = null;
		toast.success(`"${rec.title}" deleted permanently.`);
	} catch (e) {
		archived = snapshot;
		toast.error((e as Error)?.message ?? "Couldn't delete recast.");
	} finally {
		deleting = false;
	}
}
</script>

<svelte:head>
	<title>Archive - Recast Dashboard</title>
</svelte:head>

<PageHeader
	icon={Archive}
	title="Archive"
	subtitle="Recasts whose cloud files were archived after inactivity."
/>

<div class="mt-8" in:fly={{ y: 12, duration: 480, easing: cubicOut }}>
	{#if archived.length > 0}
		<!-- One explanation for the page, not one per card. -->
		<div class="mb-5 flex flex-wrap items-start justify-between gap-x-6 gap-y-2 border-y border-border-low py-4">
			<p class="max-w-xl text-body-sm text-muted-foreground">
				These lost their cloud file after 14 days without views, so only the details remain.
				Re-share from the desktop app to bring one back, or delete it for good.
			</p>
			<p class="shrink-0 text-body-sm tabular-nums text-muted-foreground">
				{archived.length}
				{archived.length === 1 ? "recast" : "recasts"} · {formatBytes(totalBytes)} · purged after 16 days
			</p>
		</div>
		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
			{#each archived as rec (rec.id)}
				<div
					animate:flip={{ duration: 320, easing: cubicOut }}
					in:scale={{ start: 0.97, duration: 300, easing: cubicOut }}
					out:scale={{ start: 0.97, duration: 170, easing: cubicOut }}
				>
					<ArchivedCard recast={rec} ondelete={() => (confirmDelete = rec)} />
				</div>
			{/each}
		</div>
	{:else}
		<EmptyState
			icon={Archive}
			title="Nothing archived"
			description="Archived recasts will appear here when an inactive file ages out of cloud storage."
		/>
	{/if}
</div>

<ConfirmDialog
	bind:open={() => confirmDelete !== null, (v) => !v && (confirmDelete = null)}
	title="Delete permanently?"
	description={`“${confirmDelete?.title ?? ""}” and its viewer history are removed for good. The cloud file is already gone.`}
	confirmLabel="Delete permanently"
	busy={deleting}
	onconfirm={() => confirmDelete && deleteArchived(confirmDelete)}
/>
