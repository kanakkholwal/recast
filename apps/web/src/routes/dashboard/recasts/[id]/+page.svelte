<script lang="ts">
import * as api from "$lib/dashboard/api";
import RecastShares, { type ShareRow } from "$lib/dashboard/components/RecastShares.svelte";
import { RecastPlayer } from "@recast/player";
import { toast } from "@recast/ui/sonner";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import type { PageData } from "./$types";

let { data }: { data: PageData } = $props();

const recast = $derived(data.recast);

// Local share list so revoke/create reflect immediately, re-seeded from the
// loader on navigation / invalidateAll.
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
		className="w-full overflow-hidden rounded-2xl ring-1 ring-inset ring-border-low/40"
	/>
</div>

<!-- Share links -->
<div class="mt-6">
	<RecastShares {shares} recastId={recast.id} onrevoke={revokeShare} />
</div>
