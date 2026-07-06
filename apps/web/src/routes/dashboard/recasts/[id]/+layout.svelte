<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import * as api from "$lib/dashboard/api";
	import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
	import RecastTabs from "$lib/dashboard/components/RecastTabs.svelte";
	import { POSTER_ACCEPT, replacePoster } from "$lib/dashboard/poster";
	import { formatRecastSubtitle } from "$lib/dashboard/recast-detail.logic";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import { ArrowLeft, Check, Copy, ImagePlus, Loader2 } from "@lucide/svelte";
	import type { Snippet } from "svelte";
	import type { LayoutData } from "./$types";

	let { data, children }: { data: LayoutData; children: Snippet } = $props();

	const recast = $derived(data.recast);
	const subtitle = $derived(formatRecastSubtitle(recast));

	// Copy-link feedback lives on the button itself (tick + "Copied link") rather
	// than a toast, so the action confirms right where it happened.
	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	async function copyLink() {
		try {
			let slug = data.shares[0]?.slug ?? null;
			if (!slug) {
				const { slug: newSlug } = await api.shareRecast(recast.id);
				slug = newSlug;
				await invalidateAll();
			}
			await navigator.clipboard.writeText(`${location.origin}/share/${slug}`);
			copied = true;
			clearTimeout(copiedTimer);
			copiedTimer = setTimeout(() => (copied = false), 2000);
		} catch (e) {
			toast.error((e as Error)?.message ?? "Couldn't copy the link.");
		}
	}

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
	class="mb-4 inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
>
	<ArrowLeft class="size-3.5" />
	Library
</a>

<PageHeader title={recast.title} {subtitle}>
	<Button variant="outline" size="sm" class="gap-2" disabled={replacingPoster} onclick={pickPoster}>
		{#if replacingPoster}
			<Loader2 class="size-3.5 animate-spin" />
			Saving…
		{:else}
			<ImagePlus class="size-3.5" />
			Change cover
		{/if}
	</Button>
	<Button variant="outline" size="sm" class="gap-2" onclick={copyLink}>
		{#if copied}
			<Check class="size-3.5 text-success" />
			Copied link
		{:else}
			<Copy class="size-3.5" />
			Copy link
		{/if}
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
