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
		LoaderCircle,
		Rocket,
		ShieldCheck,
	} from "@recast/icons";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let { data } = $props();

	let checkingOut = $state(false);
	let openingPortal = $state(false);

	const accountPlan = $derived(data.subscription?.plan ?? "free");
	const workspacePlan = $derived(data.quota?.plan ?? data.activeOrganization.plan ?? accountPlan);
	const currentPlan = $derived(data.plans.find((plan) => plan.id === accountPlan) ?? data.plans[0]);
	const proPlan = $derived(data.plans.find((plan) => plan.id === "pro"));
	const subscriptionStatus = $derived(data.subscription?.status ?? "free");
	const canUsePortal = $derived(Boolean(data.subscription?.polarCustomerId));
	const periodEndLabel = $derived(
		data.subscription?.currentPeriodEnd
			? new Date(data.subscription.currentPeriodEnd).toLocaleDateString("en-US", {
					month: "short",
					day: "numeric",
					year: "numeric",
				})
			: null,
	);

	function planName(plan: string) {
		return `${plan.charAt(0).toUpperCase()}${plan.slice(1)}`;
	}

	function activeSharesLabel(value: number | null) {
		return value == null ? "Unlimited active shares" : `${value} active shares`;
	}

	async function startCheckout() {
		if (checkingOut || !data.billingConfigured) return;
		checkingOut = true;
		try {
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

<div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_340px]">
	<div class="space-y-4" in:fly={{ y: 14, duration: 420, easing: cubicOut }}>
		<SettingsSection
			icon={CreditCard}
			title="Plan & billing"
			description="Subscription state and workspace limits for the active workspace."
			accent={accountPlan === "pro"}
		>
			{#snippet badge()}
				<Badge variant={accountPlan === "pro" ? "secondary" : "outline"}>
					{currentPlan?.name ?? planName(accountPlan)}
				</Badge>
			{/snippet}

			<div class="grid gap-3 sm:grid-cols-3">
				<div class="rounded-lg border border-border-low/70 bg-background/55 p-4">
					<p class="text-xs font-medium text-muted-foreground">Account plan</p>
					<p class="mt-1 text-lg font-semibold text-foreground">{currentPlan?.name}</p>
					<p class="text-xs text-muted-foreground">${currentPlan?.priceUsd ?? 0}/month</p>
				</div>
				<div class="rounded-lg border border-border-low/70 bg-background/55 p-4">
					<p class="text-xs font-medium text-muted-foreground">Workspace plan</p>
					<p class="mt-1 text-lg font-semibold text-foreground">{planName(workspacePlan)}</p>
					<p class="text-xs text-muted-foreground">{data.activeOrganization.name}</p>
				</div>
				<div class="rounded-lg border border-border-low/70 bg-background/55 p-4">
					<p class="text-xs font-medium text-muted-foreground">Billing status</p>
					<p class="mt-1 text-lg font-semibold text-foreground">{planName(subscriptionStatus)}</p>
					<p class="text-xs text-muted-foreground">
						{#if periodEndLabel}
							{data.subscription?.cancelAtPeriodEnd ? "Ends" : "Renews"} {periodEndLabel}
						{:else}
							No paid subscription
						{/if}
					</p>
				</div>
			</div>

			<div class="mt-4 flex flex-wrap gap-2">
				{#if accountPlan === "pro" && canUsePortal}
					<Button onclick={openPortal} disabled={openingPortal} size="sm" class="gap-2">
						{#if openingPortal}
							<LoaderCircle class="size-3.5 animate-spin" />
						{:else}
							<ArrowUpRight class="size-3.5" />
						{/if}
						Manage billing
					</Button>
				{:else if proPlan}
					<Button
						onclick={startCheckout}
						disabled={checkingOut || !data.billingConfigured}
						size="sm"
						class="gap-2"
					>
						{#if checkingOut}
							<LoaderCircle class="size-3.5 animate-spin" />
						{:else}
							<Rocket class="size-3.5" />
						{/if}
						Upgrade to Pro
					</Button>
				{/if}
				<Button href="/pricing" variant="outline" size="sm">Compare plans</Button>
			</div>

			{#if !data.billingConfigured}
				<p class="mt-3 text-xs text-muted-foreground">
					Billing checkout is disabled until Polar environment variables are configured.
				</p>
			{/if}
		</SettingsSection>

		<SettingsSection
			icon={ShieldCheck}
			title="Current plan features"
			description="Features enforced by the current account subscription."
			tone="muted"
		>
			<div class="grid gap-2 sm:grid-cols-2">
				{#each [
					activeSharesLabel(currentPlan?.limits.activeShares ?? 10),
					currentPlan?.limits.analytics ? "Share analytics" : "Basic share stats",
					currentPlan?.limits.passwordProtection ? "Password protection" : "Public links only",
					currentPlan?.limits.linkExpiry ? "Link expiry controls" : "No link expiry controls",
					currentPlan?.limits.customBranding ? "Custom branding" : "Recast watermark",
				] as feature (feature)}
					<div class="flex items-center gap-2 rounded-lg border border-border-low/60 bg-background/45 px-3 py-2 text-sm text-foreground">
						<Check class="size-3.5 text-primary" />
						<span>{feature}</span>
					</div>
				{/each}
			</div>
		</SettingsSection>
	</div>

	<div in:fly={{ y: 14, duration: 420, delay: 80, easing: cubicOut }}>
		<SettingsSection
			icon={Crown}
			title="Pro"
			description="Best fit for active cloud sharing."
			accent
		>
			{#snippet badge()}
				<Badge variant="secondary">${proPlan?.priceUsd ?? 10}/mo</Badge>
			{/snippet}

			<div class="space-y-3 text-sm">
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Active shares</span>
					<span class="font-medium text-foreground">{activeSharesLabel(proPlan?.limits.activeShares ?? null)}</span>
				</div>
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Analytics</span>
					<span class="font-medium text-foreground">Included</span>
				</div>
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Watermark</span>
					<span class="font-medium text-foreground">Removed</span>
				</div>
				<div class="flex items-center justify-between gap-4">
					<span class="text-muted-foreground">Access controls</span>
					<span class="font-medium text-foreground">Password + expiry</span>
				</div>
			</div>
		</SettingsSection>
	</div>
</div>
