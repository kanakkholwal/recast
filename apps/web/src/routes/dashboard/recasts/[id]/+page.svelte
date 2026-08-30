<script lang="ts">
import { RecastPlayer } from "@recast/player";
import { toast } from "@recast/ui/sonner";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import * as api from "$lib/dashboard/api";
import RecastShares, { type ShareRow } from "$lib/dashboard/components/RecastShares.svelte";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

const recast = $derived(data.recast);

// A local list so revoke and create reflect immediately, re-seeded from the loader on navigation.
let shares = $state<ShareRow[]>([]);
$effect(() => {
	const next = data.shares;
	untrack(() => (shares = next));
});

async function revokeShare(slug: string) {
	const snapshot = shares;
	shares = shares.filter((s) => s.slug !== slug);
	try {
		await api.deleteShare(slug);
		toast.success("Share link revoked.");
	} catch (e) {
		shares = snapshot;
		toast.error((e as Error)?.message ?? "Couldn't revoke the link.");
	}
}
</script>

<svelte:head>
	<title>{recast.title} - Recast</title>
</svelte:head>

<!-- Player — our RecastPlayer, playing inline so it feels native to the app. -->
<div in:fly={{ y: 12, duration: 480, easing: cubicOut }}>
	<RecastPlayer
		src={recast.videoUrl}
		poster={recast.posterUrl || null}
		title={recast.title}
		aspectRatio={recast.width && recast.height ? `${recast.width} / ${recast.height}` : "16 / 9"}
		className="w-full overflow-hidden rounded-xl border border-border-low"
	/>
</div>

<!-- The description is what viewers read under the player, and it was editable
     from the header without ever being shown back here. -->
{#if recast.description}
	<section class="surface mt-4 p-5" in:fly={{ y: 12, duration: 460, delay: 80, easing: cubicOut }}>
		<h2 class="font-display text-body font-medium text-foreground">Description</h2>
		<p class="mt-2 whitespace-pre-line text-body-sm text-muted-foreground">
			{recast.description}
		</p>
	</section>
{/if}

<!-- Share links -->
<div class="mt-4" in:fly={{ y: 12, duration: 460, delay: 140, easing: cubicOut }}>
	<RecastShares {shares} recastId={recast.id} onrevoke={revokeShare} />
</div>
