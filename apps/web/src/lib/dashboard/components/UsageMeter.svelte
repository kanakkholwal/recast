<script lang="ts">
import { barWidth, formatBytes } from "$lib/dashboard/format";
import { quotaStore } from "$lib/dashboard/store.svelte";
import { type UsageTone, usageView } from "./UsageMeter.logic";
import { Gauge, HardDrive, Link2 } from "@recast/icons";

// Reactive snapshot pulled from the layout-injected quota. Every plan has a
// concrete cap, so all three bars read the same way on every tier.
const view = $derived(usageView(quotaStore.value));

const barFill: Record<UsageTone, string> = {
	neutral: "bg-foreground",
	warning: "bg-warning",
	critical: "bg-destructive",
};
</script>

<section class="surface flex flex-col gap-5 p-5">
	<div class="flex items-center justify-between gap-2">
		<div class="flex items-center gap-2">
			<HardDrive class="size-4 text-muted-foreground" />
			<h2 class="font-display text-body font-medium text-foreground">Workspace usage</h2>
		</div>
		<span class="shrink-0 rounded-full border border-border-low px-2 py-0.5 text-caption font-medium text-muted-foreground">
			{view.planLabel}
		</span>
	</div>

	<!-- Storage -->
	<div>
		<div class="flex items-center justify-between gap-3 text-body-sm">
			<span class="font-medium text-foreground">Storage</span>
			<span class="shrink-0 text-caption tabular-nums text-muted-foreground">
				{formatBytes(view.usedBytes)} / {view.storageLimit != null ? formatBytes(view.storageLimit) : "—"}
			</span>
		</div>
		<div
			class="mt-2 h-1.5 overflow-hidden rounded-full bg-paper"
			role="progressbar"
			aria-label="Storage used"
			aria-valuenow={view.storagePct}
			aria-valuemin={0}
			aria-valuemax={100}
		>
			<div
				class="h-full rounded-full transition-[width] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)] {barFill[view.storageTone]}"
				style="width: {barWidth(view.storagePct)}%"
			></div>
		</div>
		<p class="mt-1.5 text-caption text-muted-foreground">
			{view.storageStatus}
		</p>
	</div>

	<!-- Delivery: the metered cost driver, and the cap most workspaces hit first -->
	<div>
		<div class="flex items-center justify-between gap-3 text-body-sm">
			<span class="font-medium text-foreground">
				<Gauge class="-mt-0.5 mr-1 inline size-3 text-muted-foreground" />
				Delivered this month
			</span>
			<span class="shrink-0 text-caption tabular-nums text-muted-foreground">
				{formatBytes(view.deliveryBytes)} / {view.deliveryLimit != null
					? formatBytes(view.deliveryLimit)
					: "—"}
			</span>
		</div>
		<div
			class="mt-2 h-1.5 overflow-hidden rounded-full bg-paper"
			role="progressbar"
			aria-label="Delivery used this month"
			aria-valuenow={view.deliveryPct}
			aria-valuemin={0}
			aria-valuemax={100}
		>
			<div
				class="h-full rounded-full transition-[width] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)] {barFill[view.deliveryTone]}"
				style="width: {barWidth(view.deliveryPct)}%"
			></div>
		</div>
		<p class="mt-1.5 text-caption text-muted-foreground">
			{view.deliveryStatus}
		</p>
	</div>

	<!-- Active links -->
	<div>
		<div class="flex items-center justify-between gap-3 text-body-sm">
			<span class="font-medium text-foreground">
				<Link2 class="-mt-0.5 mr-1 inline size-3 text-muted-foreground" />
				Active recasts
			</span>
			<span class="shrink-0 text-caption tabular-nums text-muted-foreground">
				{view.activeRecasts} / {view.linksLimit ?? "—"}
			</span>
		</div>
		<div
			class="mt-2 h-1.5 overflow-hidden rounded-full bg-paper"
			role="progressbar"
			aria-label="Active recasts used"
			aria-valuenow={view.linksPct}
			aria-valuemin={0}
			aria-valuemax={100}
		>
			<div
				class="h-full rounded-full transition-[width] duration-700 ease-[cubic-bezier(0.625,0.05,0,1)] {barFill[view.linksTone]}"
				style="width: {barWidth(view.linksPct)}%"
			></div>
		</div>
		<p class="mt-1.5 text-caption text-muted-foreground">
			{view.linksStatus}
		</p>
	</div>
</section>
