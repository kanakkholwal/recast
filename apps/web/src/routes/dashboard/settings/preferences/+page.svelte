<script lang="ts">
import { page } from "$app/state";
import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
import { quotaStore } from "$lib/dashboard/store.svelte";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import {
	Building2,
	HardDrive,
	Monitor,
	Moon,
	Settings2,
	ShieldCheck,
	Sun,
	Users,
} from "@recast/icons";
import { resetMode, setMode, userPrefersMode } from "@recast/ui/theme";
import { cn } from "@recast/ui/utils";
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

// Appearance belongs on the preferences page, not only in the profile menu.
const themes = [
	{ id: "light" as const, label: "Light", icon: Sun },
	{ id: "dark" as const, label: "Dark", icon: Moon },
	{ id: "system" as const, label: "System", icon: Monitor },
];

function chooseTheme(id: "light" | "dark" | "system") {
	if (id === "system") resetMode();
	else setMode(id);
}

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
				<div class="rounded-lg border border-border-low bg-paper p-4">
					<div class="flex items-center gap-2 text-caption text-muted-foreground">
						<Building2 class="size-3.5" />
						Active workspace
					</div>
					<p class="mt-2 text-body-sm font-medium text-foreground">{activeOrganization?.name}</p>
					<p class="mt-0.5 text-caption text-muted-foreground">/{activeOrganization?.slug}</p>
				</div>
				<div class="rounded-lg border border-border-low bg-paper p-4">
					<div class="flex items-center gap-2 text-caption text-muted-foreground">
						<ShieldCheck class="size-3.5" />
						Login default
					</div>
					<div class="mt-2 flex items-center justify-between gap-3">
						<p class="text-body-sm font-medium text-foreground">
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
					<Button href="/dashboard/team" size="sm" variant="dark" class="gap-2">
						<ShieldCheck class="size-3.5" />
						Set as default in workspace
					</Button>
				{/if}
			</div>
		</SettingsSection>
	</div>

	<div class="space-y-4" in:fly={{ y: 14, duration: 420, delay: 80, easing: cubicOut }}>
		<SettingsSection
			icon={Sun}
			title="Appearance"
			description="Applies to this browser. The desktop app keeps its own setting."
		>
			<div
				class="grid grid-cols-3 gap-2"
				role="radiogroup"
				aria-label="Colour theme"
			>
				{#each themes as t (t.id)}
					{@const active = (userPrefersMode.current ?? "system") === t.id}
					{@const Icon = t.icon}
					<button
						type="button"
						role="radio"
						aria-checked={active}
						onclick={() => chooseTheme(t.id)}
						class={cn(
							"flex flex-col items-center gap-2 rounded-lg border px-3 py-3 text-caption font-medium transition-colors duration-200 motion-reduce:transition-none",
							active
								? "border-border-strong bg-paper text-foreground"
								: "border-border-low text-muted-foreground hover:bg-paper hover:text-foreground",
						)}
					>
						<Icon class="size-4" />
						{t.label}
					</button>
				{/each}
			</div>
		</SettingsSection>

		<SettingsSection
			icon={HardDrive}
			title="Workspace limits"
			description="{planLabel} plan limits for this workspace."
		>
			<div class="space-y-3 text-body-sm">
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Storage used</span>
					<span class="font-medium text-foreground">{quota?.storagePctUsed ?? 0}%</span>
				</div>
				<div
					class="h-2 overflow-hidden rounded-full bg-paper"
					role="progressbar"
					aria-label="Storage used"
					aria-valuenow={quota?.storagePctUsed ?? 0}
					aria-valuemin={0}
					aria-valuemax={100}
				>
					<div
						class="h-full rounded-full bg-foreground transition-[width] duration-500 ease-[cubic-bezier(0.625,0.05,0,1)] motion-reduce:transition-none"
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
