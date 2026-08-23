<script lang="ts" module>
export type ShareRow = {
	slug: string;
	visibility: string;
	viewsCount: number;
	hasPassword: boolean;
	expiresAt: number | null;
	createdAt: number;
};
</script>

<script lang="ts">
	import { browser } from "$app/environment";
	import { formatCount, formatExpiry } from "$lib/dashboard/format";
	import { shareDialog } from "$lib/dashboard/share-dialog.svelte";
	import {
		CalendarClock,
		Copy,
		ExternalLink,
		Globe,
		Link2,
		Lock,
		Plus,
		Share2,
		Trash2,
		UserCheck,
		Users,
	} from "@recast/icons";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import EmptyState from "./EmptyState.svelte";

	let {
		shares,
		recastId,
		onrevoke,
	}: {
		shares: ShareRow[];
		recastId: string;
		onrevoke: (slug: string) => void;
	} = $props();

	// Newest first, so a freshly created link surfaces at the top.
	const ordered = $derived([...shares].sort((a, b) => b.createdAt - a.createdAt));

	function accessMeta(v: string): { label: string; icon: typeof Globe } {
		if (v === "workspace" || v === "team") return { label: "Workspace members", icon: Users };
		if (v === "selected") return { label: "Specific people", icon: UserCheck };
		if (v === "private") return { label: "Only workspace admins", icon: Lock };
		return { label: "Anyone with the link", icon: Globe };
	}

	async function copy(slug: string) {
		if (!browser) return;
		try {
			await navigator.clipboard.writeText(`${location.origin}/share/${slug}`);
			toast.success("Share link copied to clipboard.");
		} catch {
			toast.error("Couldn't copy the link.");
		}
	}
</script>

<section class="surface">
	<header class="flex items-center justify-between gap-4 border-b border-border-low px-5 py-3.5">
		<div class="flex items-center gap-2">
			<Share2 class="size-4 text-muted-foreground" />
			<h2 class="font-display text-body font-medium text-foreground">Share links</h2>
		</div>
		{#if ordered.length > 0}
			<Button variant="outline" size="sm" class="gap-1.5" onclick={() => shareDialog.show(recastId)}>
				<Plus class="size-3.5" />
				New link
			</Button>
		{/if}
	</header>

	{#if ordered.length === 0}
		<EmptyState
			bordered={false}
			icon={Link2}
			title="Not shared yet"
			description="Choose who can see it, then create a link."
		>
			<Button size="sm" variant="dark" class="gap-2" onclick={() => shareDialog.show(recastId)}>
				<Link2 class="size-3.5" />
				Share this recast
			</Button>
		</EmptyState>
	{:else}
		<ul class="divide-y divide-border-low">
			{#each ordered as s (s.slug)}
				{@const meta = accessMeta(s.visibility)}
				{@const exp = s.expiresAt ? formatExpiry(s.expiresAt) : null}
				<li class="flex items-center gap-3 px-5 py-3 {exp?.expired ? 'opacity-60' : ''}">
					<meta.icon class="size-4 shrink-0 text-muted-foreground" />
					<div class="min-w-0 flex-1">
						<div class="flex flex-wrap items-center gap-x-2 gap-y-1">
							<span class="truncate text-body-sm font-medium text-foreground">{meta.label}</span>
							{#if s.hasPassword}
								<span class="inline-flex items-center gap-1 text-caption text-muted-foreground">
									<Lock class="size-2.5" /> Password
								</span>
							{/if}
							{#if exp}
								<span
									class="inline-flex items-center gap-1 text-caption {exp.expired
										? 'text-destructive'
										: 'text-muted-foreground'}"
								>
									<CalendarClock class="size-2.5" />
									{exp.label}
								</span>
							{/if}
						</div>
						<div class="mt-0.5 flex flex-wrap items-center gap-x-2 text-caption text-muted-foreground">
							<span class="truncate">/share/{s.slug}</span>
							<span aria-hidden="true">·</span>
							<span class="tabular-nums">{formatCount(s.viewsCount)} views</span>
						</div>
					</div>
					<div class="flex shrink-0 items-center gap-1">
						<Button
							variant="ghost"
							size="icon"
							class="size-8"
							aria-label="Copy link"
							onclick={() => copy(s.slug)}
						>
							<Copy class="size-3.5" />
						</Button>
						<a
							href={`/share/${s.slug}`}
							target="_blank"
							rel="noreferrer"
							aria-label="Open share page"
							class="grid size-8 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-paper hover:text-foreground motion-reduce:transition-none"
						>
							<ExternalLink class="size-3.5" />
						</a>
						<Button
							variant="ghost"
							size="icon"
							class="size-8 text-muted-foreground hover:text-destructive"
							aria-label="Revoke link"
							onclick={() => onrevoke(s.slug)}
						>
							<Trash2 class="size-3.5" />
						</Button>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>
