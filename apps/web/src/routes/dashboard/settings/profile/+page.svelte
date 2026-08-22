<script lang="ts">
import { invalidateAll } from "$app/navigation";
import { page } from "$app/state";
import { authClient } from "$lib/auth/client";
import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
import { settingsStore } from "$lib/dashboard/store.svelte";
import { BadgeCheck, LoaderCircle, MailWarning, RefreshCw, User } from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import { toast } from "@recast/ui/sonner";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

type LayoutData = { user?: { name: string; email: string; emailVerified?: boolean } };
const layoutUser = $derived((page.data as LayoutData).user ?? null);
const verified = $derived(Boolean(layoutUser?.emailVerified));
const accountName = $derived(layoutUser?.name || layoutUser?.email || "Recast user");
const accountEmail = $derived(layoutUser?.email ?? "");

let resending = $state(false);
let saving = $state(false);
let name = $state(untrack(() => (page.data as LayoutData).user?.name ?? ""));

const dirty = $derived(name.trim() !== (layoutUser?.name ?? "") && name.trim().length > 0);

async function saveName(e: SubmitEvent) {
	e.preventDefault();
	if (!dirty || saving) return;
	saving = true;
	try {
		const { error } = await authClient.updateUser({ name: name.trim() });
		if (error) throw new Error(error.message ?? "Couldn't save your name.");
		// The header, sidebar and greeting all read the local store.
		settingsStore.value.profile.name = name.trim();
		await invalidateAll();
		toast.success("Name updated.");
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't save your name.");
	} finally {
		saving = false;
	}
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
		<!-- Editable, because a settings page that can only be read is a dead end.
		     Email stays fixed: changing it is an auth flow, not a text field. -->
		<form class="grid gap-4 sm:grid-cols-2" onsubmit={saveName}>
			<Label class="block">
				<span class="mb-1.5 block text-body-sm font-medium text-foreground">Display name</span>
				<Input
					bind:value={name}
					name="name"
					autocomplete="name"
					placeholder={accountName}
					class="h-9 border-border-low bg-background"
					required
				/>
				<span class="mt-1.5 block text-caption text-muted-foreground">
					Shown on shared recasts and to your team.
				</span>
			</Label>

			<div class="rounded-lg border border-border-low bg-paper p-4">
				<div class="flex items-start justify-between gap-3">
					<div class="min-w-0">
						<p class="text-caption text-muted-foreground">Email</p>
						<p class="mt-1 truncate text-body-sm font-medium text-foreground">{accountEmail}</p>
					</div>
					{#if verified}
						<Badge variant="outline" class="gap-1 text-success">
							<BadgeCheck class="size-3" />
							Verified
						</Badge>
					{:else}
						<Badge variant="outline" class="gap-1 text-warning">
							<MailWarning class="size-3" />
							Unverified
						</Badge>
					{/if}
				</div>
			</div>

			<div class="sm:col-span-2">
				<Button type="submit" size="sm" variant="dark" disabled={!dirty || saving} class="gap-2">
					{#if saving}
						<LoaderCircle class="size-3.5 animate-spin" />
					{/if}
					{saving ? "Saving…" : "Save name"}
				</Button>
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
			description="Confirm {accountEmail} to use dashboard actions."
		>
			<div class="flex flex-wrap items-center gap-3">
				<Button onclick={resendVerification} disabled={resending} size="sm" variant="dark" class="gap-2">
					{#if resending}
						<LoaderCircle class="size-3.5 animate-spin" />
					{:else}
						<RefreshCw class="size-3.5" />
					{/if}
					{resending ? "Sending…" : "Send verification email"}
				</Button>
				<span class="text-body-sm text-muted-foreground">Link valid for 24 hours.</span>
			</div>
		</SettingsSection>
	{/if}
</div>
