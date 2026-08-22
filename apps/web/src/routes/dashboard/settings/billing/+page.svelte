<script lang="ts">
import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
import { authClient } from "$lib/auth/client";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import {
	ArrowUpRight,
	Check,
	CreditCard,
	Crown,
	Gauge,
	LoaderCircle,
	Minus,
	Rocket,
	ShieldCheck,
	Users,
} from "@recast/icons";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { barWidth } from "$lib/dashboard/format";
import { approxViews, formatBytes, formatUsd, meterTone, seatView } from "./billing.logic";

let { data } = $props();

let checkingOut = $state(false);
let openingPortal = $state(false);

const plan = $derived(data.plan);
const isPaid = $derived(plan.id !== "free");
const canUsePortal = $derived(Boolean(data.subscription?.polarCustomerId));
const status = $derived<string>(data.subscription?.status ?? "none");

// Enterprise is provisioned by contract, so it has no Polar subscription.
// Rendering that as "No subscription" reads like the plan failed to apply.
const isAgreement = $derived(isPaid && data.currentMonthlyUsd == null);

const STATUS_LABEL: Record<string, string> = {
	active: "Active",
	trialing: "Trial",
	past_due: "Past due",
	canceled: "Canceled",
	incomplete: "Incomplete",
	unpaid: "Unpaid",
};

const statusLabel = $derived(
	status !== "none"
		? (STATUS_LABEL[status] ?? status.replace(/_/g, " "))
		: isAgreement
			? "By agreement"
			: "No subscription",
);

const seats = $derived(
	seatView(data.seats, plan.seats.included, plan.seats.max, plan.seats.monthlyUsd),
);
const delivery = $derived(data.delivery);
const deliveryPct = $derived(Math.round(Math.min(1, delivery?.ratio ?? 0) * 100));
const storagePct = $derived(data.quota?.storagePctUsed ?? 0);

const periodEndLabel = $derived(
	data.subscription?.currentPeriodEnd
		? new Date(data.subscription.currentPeriodEnd).toLocaleDateString("en-US", {
				month: "short",
				day: "numeric",
				year: "numeric",
			})
		: null,
);

// The rail only carries rows that say something. An agreement has no monthly
// figure and no renewal date, and blank placeholders read as broken data.
const railRows = $derived(
	[
		data.currentMonthlyUsd != null
			? { label: "Monthly total", value: `${formatUsd(data.currentMonthlyUsd)}/mo` }
			: null,
		{ label: "Creators billed", value: `${seats.used} of ${seats.max}` },
		periodEndLabel
			? {
					label: data.subscription?.cancelAtPeriodEnd ? "Ends" : "Renews",
					value: periodEndLabel,
				}
			: null,
	].filter((row) => row !== null),
);

const featureRows = $derived([
	{ label: "Watch analytics", on: plan.features.analytics },
	{ label: "Password protection", on: plan.features.passwordProtection },
	{ label: "Link expiry controls", on: plan.features.linkExpiry },
	{ label: "Per-viewer access", on: plan.features.perViewerAccess },
	{ label: "Custom branding", on: plan.features.customBranding },
]);

// The workspace must be pinned before Polar redirects, or the webhook can't
// tell which workspace the payment belongs to.
async function startCheckout() {
	if (checkingOut || !data.billingConfigured || !data.isOwner) return;
	checkingOut = true;
	try {
		const res = await fetch("/api/billing/checkout-intent", {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ workspaceId: data.workspace.id, seats: seats.used }),
		});
		if (!res.ok) {
			const body = (await res.json().catch(() => ({}))) as { message?: string };
			throw new Error(body.message ?? "Couldn't start checkout.");
		}
		const { error } = await authClient.checkout({ slug: "pro" });
		if (error) throw new Error(error.message ?? "Couldn't start checkout.");
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't start checkout.");
	} finally {
		checkingOut = false;
	}
}

async function openPortal() {
	if (openingPortal || !canUsePortal) return;
	openingPortal = true;
	try {
		const { error } = await authClient.customer.portal();
		if (error) throw new Error(error.message ?? "Couldn't open billing portal.");
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't open billing portal.");
	} finally {
		openingPortal = false;
	}
}
</script>

<div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px] lg:items-start">
	<div class="space-y-4" in:fly={{ y: 14, duration: 420, easing: cubicOut }}>
		<SettingsSection
			icon={CreditCard}
			title="Plan & billing"
			description="Plans are billed per workspace, not per account. This is {data.workspace.name}."
			accent={isPaid}
		>
			{#snippet badge()}
				<Badge variant={isPaid ? "secondary" : "outline"}>{plan.name}</Badge>
			{/snippet}

			<div class="grid gap-3 sm:grid-cols-3">
				<div class="rounded-lg border border-border-low bg-paper p-4">
					<p class="text-caption text-muted-foreground">Workspace plan</p>
					<p class="mt-1 text-subheading font-medium text-foreground">{plan.name}</p>
					<p class="text-body-sm text-muted-foreground">
						{#if data.currentMonthlyUsd == null}
							Billed by agreement
						{:else}
							{formatUsd(data.currentMonthlyUsd)}/month
						{/if}
					</p>
				</div>
				<div class="rounded-lg border border-border-low bg-paper p-4">
					<p class="text-caption text-muted-foreground">Creators</p>
					<p class="mt-1 text-subheading font-medium text-foreground">
						{seats.used} / {seats.max}
					</p>
					<p class="text-body-sm text-muted-foreground">
						{#if seats.billable > 0 && seats.extraUsd > 0}
							{seats.included} included, {seats.billable} × {formatUsd(seats.extraUsd)}
						{:else}
							{seats.included} included
						{/if}
					</p>
				</div>
				<div class="rounded-lg border border-border-low bg-paper p-4">
					<p class="text-caption text-muted-foreground">Billing status</p>
					<p class="mt-1 text-subheading font-medium text-foreground">{statusLabel}</p>
					<p class="text-body-sm text-muted-foreground">
						{#if periodEndLabel}
							{data.subscription?.cancelAtPeriodEnd ? "Ends" : "Renews"} {periodEndLabel}
						{:else if isAgreement}
							Managed with your account contact
						{:else}
							Nothing scheduled
						{/if}
					</p>
				</div>
			</div>

			<div class="mt-4 flex flex-wrap gap-2">
				{#if isPaid && canUsePortal}
					<Button onclick={openPortal} disabled={openingPortal} size="sm" variant="dark" class="gap-2">
						{#if openingPortal}
							<LoaderCircle class="size-3.5 animate-spin" />
						{:else}
							<ArrowUpRight class="size-3.5" />
						{/if}
						Manage billing
					</Button>
				{:else if data.isOwner}
					<Button
						onclick={startCheckout}
						disabled={checkingOut || !data.billingConfigured}
						size="sm"
						variant="dark"
						class="gap-2"
					>
						{#if checkingOut}
							<LoaderCircle class="size-3.5 animate-spin" />
						{:else}
							<Rocket class="size-3.5" />
						{/if}
						Upgrade this workspace
					</Button>
				{/if}
				<Button href="/pricing" variant="outline" size="sm">Compare plans</Button>
			</div>

			{#if !data.isOwner}
				<p class="mt-3 text-body-sm text-muted-foreground">
					Only the workspace owner can change this plan.
				</p>
			{:else if !data.billingConfigured}
				<p class="mt-3 text-body-sm text-muted-foreground">
					Checkout is disabled until Polar environment variables are configured.
				</p>
			{:else if !data.billingContactIsMe}
				<p class="mt-3 text-body-sm text-muted-foreground">
					Another owner is the billing contact for this workspace.
				</p>
			{/if}
		</SettingsSection>

		<SettingsSection
			icon={Gauge}
			title="Usage this month"
			description="Delivery is what viewers stream. It resets on the 1st."
		>
			<div class="space-y-4">
				<div>
					<div class="flex items-baseline justify-between gap-4 text-body-sm">
						<span class="text-muted-foreground">Delivery</span>
						<span class="font-medium text-foreground">
							{formatBytes(delivery?.usedBytes ?? 0)} / {formatBytes(delivery?.capBytes)}
						</span>
					</div>
					<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-paper">
						<div
							class="h-full rounded-full transition-[width] duration-500 {meterTone(
								delivery?.ratio ?? 0,
							) === 'critical'
								? 'bg-destructive'
								: meterTone(delivery?.ratio ?? 0) === 'warning'
									? 'bg-warning'
									: 'bg-primary'}"
							style="width: {barWidth(deliveryPct)}%"
						></div>
					</div>
					<p class="mt-1.5 text-caption text-muted-foreground">
						{#if delivery?.capBytes == null}
							Unlimited on this plan.
						{:else if delivery.exceeded && !isPaid}
							Cap reached — shares are paused until the 1st, or upgrade to resume now.
						{:else if delivery.exceeded}
							Over the included allowance. Extra delivery bills at {formatUsd(
								data.deliveryOverageUsdPerGb,
							)}/GB.
						{:else}
							{@const views = approxViews(delivery.capBytes)}
							{deliveryPct}% used{views ? ` · about ${views} views included` : ""}
						{/if}
					</p>
				</div>

				<div>
					<div class="flex items-baseline justify-between gap-4 text-body-sm">
						<span class="text-muted-foreground">Storage</span>
						<span class="font-medium text-foreground">
							{formatBytes(data.quota?.usage.storageBytes ?? 0)} / {formatBytes(
								data.quota?.limits.storageBytes,
							)}
						</span>
					</div>
					<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-paper">
						<div
							class="h-full rounded-full bg-primary transition-[width] duration-500"
							style="width: {barWidth(storagePct)}%"
						></div>
					</div>
				</div>
			</div>
		</SettingsSection>

		<SettingsSection
			icon={ShieldCheck}
			title="What this workspace includes"
			description="Enforced server-side for every share in {data.workspace.name}."
		>
			<div class="grid gap-2 sm:grid-cols-2">
				{#each featureRows as feature (feature.label)}
					<div
						class="flex items-center gap-2 rounded-lg border border-border-low bg-paper px-3 py-2 text-body-sm {feature.on
							? 'text-foreground'
							: 'text-muted-foreground'}"
					>
						{#if feature.on}
							<Check class="size-3.5 shrink-0 text-success" aria-label="Included" />
						{:else}
							<Minus class="size-3.5 shrink-0 text-border-strong" aria-label="Not included" />
						{/if}
						<span>{feature.label}</span>
					</div>
				{/each}
			</div>
		</SettingsSection>
	</div>

	<div in:fly={{ y: 14, duration: 420, delay: 80, easing: cubicOut }}>
		{#if isPaid}
			<!-- Already paying: the rail answers "what am I on and when does it
			     renew", not "why should I buy this". The plan name is already in
			     the page header and the card above, so it carries no badge. -->
			<SettingsSection
				icon={Crown}
				title={isAgreement ? "Your agreement" : "Your subscription"}
				description={isAgreement
					? "Limits and price come from your contract."
					: "Seats bill with the workspace, not per account."}
				accent
			>
				<div class="space-y-3 text-body-sm">
					{#each railRows as row (row.label)}
						<div class="flex items-center justify-between gap-4">
							<span class="whitespace-nowrap text-muted-foreground">{row.label}</span>
							<span class="text-right font-medium tabular-nums text-foreground">{row.value}</span>
						</div>
					{/each}
				</div>

				{#if data.subscription?.cancelAtPeriodEnd}
					<p class="mt-4 rounded-lg border border-border-low bg-paper px-3 py-2.5 text-body-sm text-muted-foreground">
						Cancellation is scheduled. Shares keep working until {periodEndLabel}.
					</p>
				{:else if isAgreement}
					<p class="mt-4 rounded-lg border border-border-low bg-paper px-3 py-2.5 text-body-sm text-muted-foreground">
						Talk to your account contact to change seats or limits.
					</p>
				{:else if !data.billingContactIsMe}
					<p class="mt-4 rounded-lg border border-border-low bg-paper px-3 py-2.5 text-body-sm text-muted-foreground">
						Another owner is the billing contact, so the portal opens under their account.
					</p>
				{/if}

				{#if canUsePortal}
					<Button
						onclick={openPortal}
						disabled={openingPortal}
						variant="outline"
						size="sm"
						class="mt-4 w-full gap-2"
					>
						<ArrowUpRight class="size-3.5" />
						Invoices and payment method
					</Button>
				{/if}
			</SettingsSection>
		{:else}
			<SettingsSection icon={Crown} title="Pro" description="For teams sharing regularly." accent>
				{#snippet badge()}
					<Badge variant="secondary">{formatUsd(data.proPlan.monthlyUsd)}/mo</Badge>
				{/snippet}

				<div class="space-y-3 text-body-sm">
					<div class="flex items-center justify-between gap-4">
						<span class="whitespace-nowrap text-muted-foreground">Creators included</span>
						<span class="font-medium tabular-nums text-foreground">
							{data.proPlan.seatsIncluded}
						</span>
					</div>
					<div class="flex items-center justify-between gap-4">
						<span class="whitespace-nowrap text-muted-foreground">Each extra creator</span>
						<span class="font-medium tabular-nums text-foreground">
							{formatUsd(data.proPlan.extraSeatUsd)}/mo
						</span>
					</div>
					<div class="flex items-center justify-between gap-4">
						<span class="whitespace-nowrap text-muted-foreground">Billed annually</span>
						<span class="font-medium tabular-nums text-foreground">
							{formatUsd(data.proPlan.annualMonthlyUsd)}/mo
						</span>
					</div>
				</div>

				<div
					class="mt-4 flex items-start gap-2 rounded-lg border border-border-low bg-paper px-3 py-2.5 text-body-sm text-muted-foreground"
				>
					<Users class="mt-0.5 size-3.5 shrink-0 text-primary" />
					<span>
						Loom bills {formatUsd(18)} per person. A five-person team pays them {formatUsd(90)}
						a month and us {formatUsd(20)}.
					</span>
				</div>
			</SettingsSection>
		{/if}
	</div>
</div>
