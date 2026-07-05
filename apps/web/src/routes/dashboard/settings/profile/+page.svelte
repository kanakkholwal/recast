<script lang="ts">
	import { page } from "$app/state";
	import { authClient } from "$lib/auth/client";
	import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import {
		BadgeCheck,
		LoaderCircle,
		MailWarning,
		RefreshCw,
		User,
	} from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	type LayoutData = { user?: { name: string; email: string; emailVerified?: boolean } };
	const layoutUser = $derived(((page.data as LayoutData).user) ?? null);
	const verified = $derived(Boolean(layoutUser?.emailVerified));
	const accountName = $derived(layoutUser?.name || layoutUser?.email || "Recast user");
	const accountEmail = $derived(layoutUser?.email ?? "");

	let resending = $state(false);

	async function resendVerification() {
		if (resending) return;
		resending = true;
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.sendVerificationEmail({
						email: accountEmail,
						callbackURL: "/dashboard/settings/profile",
					});
					if (error) throw new Error(error.message ?? "Couldn't send the verification email.");
				})(),
				{
					loading: "Sending verification email…",
					success: "Sent. Check your inbox.",
					error: (err) => (err as Error)?.message ?? "Couldn't send the verification email.",
				},
			);
		} finally {
			resending = false;
		}
	}
</script>

<div class="flex flex-col gap-4" in:fly={{ y: 14, duration: 420, easing: cubicOut }}>
	<SettingsSection
		icon={User}
		title="Account"
		description="Identity used across the Recast web dashboard."
	>
		<div class="grid gap-3 sm:grid-cols-2">
			<div class="rounded-lg border border-border-low/70 bg-background/55 p-4">
				<p class="text-xs font-medium text-muted-foreground">Display name</p>
				<p class="mt-1 text-sm font-semibold text-foreground">{accountName}</p>
			</div>
			<div class="rounded-lg border border-border-low/70 bg-background/55 p-4">
				<div class="flex items-start justify-between gap-3">
					<div class="min-w-0">
						<p class="text-xs font-medium text-muted-foreground">Email</p>
						<p class="mt-1 truncate text-sm font-semibold text-foreground">{accountEmail}</p>
					</div>
					{#if verified}
						<Badge variant="outline" class="gap-1 text-success">
							<BadgeCheck class="size-3" />
							Verified
						</Badge>
					{:else}
						<Badge variant="outline" class="gap-1 text-amber-600 dark:text-amber-400">
							<MailWarning class="size-3" />
							Unverified
						</Badge>
					{/if}
				</div>
			</div>
		</div>
	</SettingsSection>

	{#if !verified}
		<!-- Soft nudge for the edge-case path: a user who somehow reached
		     settings before verifying (e.g. landing in dev mode). The
		     dashboard layout gates production paths to /verify-email. -->
		<SettingsSection
			icon={MailWarning}
			title="Email verification pending"
			description="Confirm {accountEmail} to use dashboard actions."
		>
			<div class="flex flex-wrap items-center gap-3">
				<Button onclick={resendVerification} disabled={resending} size="sm" class="gap-2">
					{#if resending}
						<LoaderCircle class="size-3.5 animate-spin" />
					{:else}
						<RefreshCw class="size-3.5" />
					{/if}
					{resending ? "Sending…" : "Send verification email"}
				</Button>
				<span class="text-xs text-muted-foreground">
					Link valid for 24 hours.
				</span>
			</div>
		</SettingsSection>
	{/if}
</div>
