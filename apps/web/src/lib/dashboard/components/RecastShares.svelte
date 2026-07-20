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

<section class="glass-card rounded-xl p-5">
	<header class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<Share2 class="size-4 text-muted-foreground" />
			<h2 class="text-sm font-semibold text-foreground">Share links</h2>
		</div>
		{#if ordered.length > 0}
			<Button  variant="default_soft" size="sm" class="gap-1.5" onclick={() => shareDialog.show(recastId)}>
				<Plus class="size-3.5" />
				New link
			</Button>
		{/if}
	</header>

	{#if ordered.length === 0}
		<div class="mt-4 flex flex-col items-start gap-2 rounded-lg border border-dashed border-border-low/70 bg-background/40 p-4">
			<p class="text-xs text-muted-foreground">
				Not shared yet. Choose who can see it, then create a link.
			</p>
			<Button size="sm" onclick={() => shareDialog.show(recastId)}>
				<Link2/>
				Share this recast
			</Button>
		</div>
	{:else}
		<ul class="mt-4 space-y-1.5">
			{#each ordered as s (s.slug)}
				{@const meta = accessMeta(s.visibility)}
				{@const exp = s.expiresAt ? formatExpiry(s.expiresAt) : null}
				<li
					class="flex items-center gap-3 rounded-lg border border-border-low/50 bg-background/40 p-2.5 {exp?.expired
						? 'opacity-60'
						: ''}"
				>
					<span class="glass-chip grid size-9 shrink-0 place-items-center rounded-lg text-muted-foreground">
						<meta.icon class="size-4" />
					</span>
					<div class="min-w-0 flex-1">
						<div class="flex flex-wrap items-center gap-1.5">
							<span class="truncate text-sm font-medium text-foreground">{meta.label}</span>
							{#if s.hasPassword}
								<span class="inline-flex items-center gap-1 rounded bg-foreground/6 px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
									<Lock class="size-2.5" /> Password
								</span>
							{/if}
							{#if exp}
								<span
									class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-semibold {exp.expired
										? 'bg-destructive/10 text-destructive'
										: 'bg-foreground/6 font-medium text-muted-foreground'}"
								>
									<CalendarClock class="size-2.5" />
									{exp.label}
								</span>
							{/if}
						</div>
						<div class="mt-0.5 flex flex-wrap items-center gap-x-2 text-[11px] text-muted-foreground">
							<span class="truncate font-mono">/share/{s.slug}</span>
							<span aria-hidden="true">·</span>
							<span class="tabular-nums">{formatCount(s.viewsCount)} views</span>
						</div>
					</div>
					<div class="flex shrink-0 items-center gap-1">
						<Button variant="ghost" size="icon" class="size-8" aria-label="Copy link" onclick={() => copy(s.slug)}>
							<Copy class="size-3.5" />
						</Button>
						<a
							href={`/share/${s.slug}`}
							target="_blank"
							rel="noreferrer"
							aria-label="Open share page"
							class="grid size-8 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
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
