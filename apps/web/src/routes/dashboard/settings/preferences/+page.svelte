<script lang="ts">
	import { page } from "$app/state";
	import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
	import { quotaStore } from "$lib/dashboard/store.svelte";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import { Building2, HardDrive, ShieldCheck, Settings2, Users } from "@recast/icons";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	type LayoutData = {
		activeOrganization?: {
			id: string;
			name: string;
			slug: string;
			plan: string;
			role: string;
			isDefault?: boolean;
		};
		user?: { defaultWorkspaceId?: string | null };
	};

	const activeOrganization = $derived((page.data as LayoutData).activeOrganization);
	const quota = $derived(quotaStore.value);
	const planLabel = $derived(
		activeOrganization
			? `${activeOrganization.plan.charAt(0).toUpperCase()}${activeOrganization.plan.slice(1)}`
			: "Free",
	);

	function formatLimit(value: number | null | undefined, unit = "") {
		if (value == null) return "Unlimited";
		return `${value.toLocaleString()}${unit}`;
	}
</script>

<div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
	<div in:fly={{ y: 14, duration: 420, easing: cubicOut }}>
		<SettingsSection
			icon={Settings2}
			title="Workspace defaults"
			description="The active workspace controls where web uploads, shares, and team settings apply."
		>
			<div class="grid gap-3 sm:grid-cols-2">
				<div class="rounded-lg border border-border-low/70 bg-background/55 p-4">
					<div class="flex items-center gap-2 text-xs font-medium text-muted-foreground">
						<Building2 class="size-3.5" />
						Active workspace
					</div>
					<p class="mt-2 text-sm font-semibold text-foreground">{activeOrganization?.name}</p>
					<p class="mt-0.5 text-xs text-muted-foreground">/{activeOrganization?.slug}</p>
				</div>
				<div class="rounded-lg border border-border-low/70 bg-background/55 p-4">
					<div class="flex items-center gap-2 text-xs font-medium text-muted-foreground">
						<ShieldCheck class="size-3.5" />
						Login default
					</div>
					<div class="mt-2 flex items-center justify-between gap-3">
						<p class="text-sm font-semibold text-foreground">
							{activeOrganization?.isDefault ? "This workspace" : "Different workspace"}
						</p>
						{#if activeOrganization?.isDefault}
							<Badge variant="secondary">Default</Badge>
						{/if}
					</div>
				</div>
			</div>
			<div class="mt-4 flex flex-wrap gap-2">
				<Button href="/dashboard/team" variant="outline" size="sm" class="gap-2">
					<Users class="size-3.5" />
					Manage workspace
				</Button>
				{#if !activeOrganization?.isDefault}
					<Button href="/dashboard/team" size="sm" class="gap-2">
						<ShieldCheck class="size-3.5" />
						Set as default
					</Button>
				{/if}
			</div>
		</SettingsSection>
	</div>

	<div in:fly={{ y: 14, duration: 420, delay: 80, easing: cubicOut }}>
		<SettingsSection
			icon={HardDrive}
			title="Workspace limits"
			description="{planLabel} plan limits for this workspace."
			tone="muted"
		>
			<div class="space-y-3 text-sm">
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Storage used</span>
					<span class="font-medium text-foreground">{quota?.storagePctUsed ?? 0}%</span>
				</div>
				<div class="h-2 overflow-hidden rounded-full bg-foreground/8">
					<div
						class="h-full rounded-full bg-primary transition-[width] duration-300"
						style:width={`${quota?.storagePctUsed ?? 0}%`}
					></div>
				</div>
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Active recasts</span>
					<span class="font-medium text-foreground">
						{quota?.usage.activeRecastsCount ?? 0} / {formatLimit(quota?.limits.activeRecasts)}
					</span>
				</div>
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Members</span>
					<span class="font-medium text-foreground">
						{quota?.usage.membersCount ?? 0} / {formatLimit(quota?.limits.members)}
					</span>
				</div>
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Max recording</span>
					<span class="font-medium text-foreground">
						{formatLimit(quota?.limits.maxDurationSec ? Math.round(quota.limits.maxDurationSec / 60) : quota?.limits.maxDurationSec, " min")}
					</span>
				</div>
			</div>
		</SettingsSection>
	</div>
</div>
