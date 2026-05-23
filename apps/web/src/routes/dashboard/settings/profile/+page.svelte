<script lang="ts">
	import { page } from "$app/state";
	import { authClient } from "$lib/auth/client";
	import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
	import { settingsStore } from "$lib/dashboard/store.svelte";
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

	const settings = settingsStore.value;

	// Pulled live from the dashboard layout's session so the badge reflects
	// the most recent server-side state, not the local profile cache.
	type LayoutData = { user?: { email: string; emailVerified?: boolean } };
	const layoutUser = $derived(((page.data as LayoutData).user) ?? null);
	const verified = $derived(Boolean(layoutUser?.emailVerified));
	const accountEmail = $derived(layoutUser?.email ?? settings.profile.email);

	let resending = $state(false);

	const inputClass =
		"rounded-lg border border-border-low/70 bg-background/80 px-3 py-2 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/60 focus:border-primary/60";

	function save(e: SubmitEvent) {
		e.preventDefault();
		settingsStore.save();
		toast.success("Profile updated.");
	}

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
					success: "Sent — check your inbox.",
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
		title="Profile"
		description="How you show up across Recast."
	>
		<form class="grid gap-4 sm:grid-cols-2" onsubmit={save}>
			<label class="flex flex-col gap-1.5">
				<span class="text-xs font-semibold text-foreground/85">Display name</span>
				<input type="text" bind:value={settings.profile.name} class={inputClass} />
			</label>
			<label class="flex flex-col gap-1.5">
				<span class="flex items-center justify-between text-xs font-semibold text-foreground/85">
					<span>Email</span>
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
				</span>
				<input type="email" bind:value={settings.profile.email} class={inputClass} />
			</label>
			<div class="sm:col-span-2">
				<Button type="submit" variant="outline" size="sm">Save changes</Button>
			</div>
		</form>
	</SettingsSection>

	{#if !verified}
		<!-- Soft nudge for the edge-case path: a user who somehow reached
		     settings before verifying (e.g. landing in dev mode). The
		     dashboard layout gates production paths to /verify-email. -->
		<SettingsSection
			icon={MailWarning}
			title="Email verification pending"
			description="Confirm {accountEmail} to unlock dashboard actions."
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
