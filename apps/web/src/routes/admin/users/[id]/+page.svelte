<script lang="ts">
import {
	ArrowLeft,
	ChevronDown,
	ClipboardList,
	Crown,
	Key,
	LoaderCircle,
	LogOut,
	ShieldOff,
	Trash2,
	UserCog,
} from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import * as Collapsible from "@recast/ui/collapsible";
import * as Dialog from "@recast/ui/dialog";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import * as Select from "@recast/ui/select";
import { Skeleton } from "@recast/ui/skeleton";
import { toast } from "@recast/ui/sonner";
import { untrack } from "svelte";
import { enhance } from "$app/forms";
import { goto } from "$app/navigation";
import { authClient } from "$lib/auth/client";
import InlineError from "$lib/components/InlineError.svelte";
import { enhanceAction } from "$lib/forms/enhance";

let { data } = $props();

const t = $derived(data.target);

// Seeded from the initial load, so it doesn't reset itself when a form action returns and re-runs the load.
let role = $state(untrack(() => data.target.role ?? "user"));
let status = $state(untrack(() => data.target.status ?? "active"));
let name = $state(untrack(() => data.target.name ?? ""));

let banReason = $state("");
let banDays = $state("");
let newPassword = $state("");

let confirmDelete = $state(false);
let confirmBan = $state(false);

// Per-action in-flight tracking.
let impersonating = $state(false);
let savingProfile = $state(false);
let settingRole = $state(false);
let settingStatus = $state(false);
let unbanning = $state(false);
let revokingAll = $state(false);
let revokingSessionToken = $state<string | null>(null);
let settingPassword = $state(false);
let deleting = $state(false);
let banning = $state(false);

async function impersonate() {
	if (impersonating) return;
	impersonating = true;
	try {
		await toast.promise(
			(async () => {
				const { error } = await authClient.admin.impersonateUser({ userId: t.id });
				if (error) throw new Error(error.message ?? "Couldn't start impersonation.");
			})(),
			{
				loading: `Starting session as ${t.email}…`,
				success: `Now acting as ${t.email}.`,
				error: (err) => (err as Error)?.message ?? "Couldn't start impersonation.",
			},
		);
		// The cookie is now the impersonation session, so leave admin and land in the impersonated user's dashboard.
		window.location.href = "/dashboard";
	} finally {
		impersonating = false;
	}
}
</script>

<a
	href="/admin/users"
	class="inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground transition-colors hover:text-foreground"
>
	<ArrowLeft class="size-3" />
	All users
</a>

<header class="mt-3 mb-8 flex flex-wrap items-start justify-between gap-4">
	<div class="min-w-0">
		<h1 class="truncate text-2xl font-semibold tracking-tight">{t.name}</h1>
		<p class="mt-1 truncate font-mono text-xs text-muted-foreground">{t.email}</p>
		<div class="mt-3 flex flex-wrap items-center gap-1.5">
			{#if t.role === "admin"}
				<Badge variant="secondary" class="gap-1"><Crown class="size-3" /> admin</Badge>
			{:else}
				<Badge variant="outline">user</Badge>
			{/if}
			{#if t.status === "pending"}
				<Badge variant="outline" class="text-amber-600 dark:text-amber-400">waitlist</Badge>
			{/if}
			{#if t.banned}
				<Badge variant="destructive" class="gap-1"><ShieldOff class="size-3" /> banned</Badge>
			{/if}
			{#if t.emailVerified}
				<Badge variant="outline">verified</Badge>
			{/if}
		</div>
	</div>
	<div class="flex gap-2">
		<Button variant="outline" size="sm" disabled={impersonating} onclick={impersonate}>
			{#if impersonating}
				<LoaderCircle class="size-3.5 animate-spin" />
			{:else}
				<UserCog class="size-3.5" />
			{/if}
			{impersonating ? "Starting…" : "Impersonate"}
		</Button>
	</div>
</header>

<div class="grid gap-6 lg:grid-cols-3">
	<!-- Profile + role/status -->
	<section class="glass-card rounded-xl p-5 lg:col-span-2">
		<h2 class="mb-4 text-sm font-semibold tracking-tight">Profile</h2>

		<form
			method="POST"
			action="?/updateProfile"
			class="space-y-3"
			use:enhance={enhanceAction({
				setBusy: (b) => (savingProfile = b),
				onSuccess: "Profile updated.",
			})}
		>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Name</span>
				<Input bind:value={name} name="name" class="h-9" />
			</Label>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Email</span>
				<Input value={t.email} readonly disabled class="h-9 font-mono" />
				<span class="mt-1 block text-[11px] text-muted-foreground">
					Email changes go through the user's own settings. Admins can't edit it.
				</span>
			</Label>
			<Button type="submit" size="sm" disabled={savingProfile} class="gap-2">
				{#if savingProfile}
					<LoaderCircle class="size-3.5 animate-spin" />
				{/if}
				{savingProfile ? "Saving…" : "Save profile"}
			</Button>
		</form>

		<hr class="my-6 border-border/40" />

		<div class="grid gap-4 sm:grid-cols-2">
			<form
				method="POST"
				action="?/setRole"
				use:enhance={enhanceAction({
					setBusy: (b) => (settingRole = b),
					onSuccess: "Role updated.",
				})}
			>
				<Label class="block">
					<span class="mb-1 block text-xs font-semibold text-foreground/85">Role</span>
					<Select.Root type="single" bind:value={role} name="role">
						<Select.Trigger class="h-9 w-full">{role}</Select.Trigger>
						<Select.Content>
							<Select.Item value="user">user</Select.Item>
							<Select.Item value="admin">admin</Select.Item>
						</Select.Content>
					</Select.Root>
				</Label>
				<Button type="submit" size="sm" disabled={settingRole} class="mt-2 w-full gap-2">
					{#if settingRole}
						<LoaderCircle class="size-3.5 animate-spin" />
					{/if}
					{settingRole ? "Saving…" : "Set role"}
				</Button>
			</form>

			<form
				method="POST"
				action="?/setStatus"
				use:enhance={enhanceAction({
					setBusy: (b) => (settingStatus = b),
					onSuccess: "Status updated.",
				})}
			>
				<Label class="block">
					<span class="mb-1 block text-xs font-semibold text-foreground/85">Status</span>
					<Select.Root type="single" bind:value={status} name="status">
						<Select.Trigger class="h-9 w-full">{status}</Select.Trigger>
						<Select.Content>
							<Select.Item value="active">active</Select.Item>
							<Select.Item value="pending">pending (waitlist)</Select.Item>
						</Select.Content>
					</Select.Root>
				</Label>
				<Button type="submit" size="sm" disabled={settingStatus} class="mt-2 w-full gap-2">
					{#if settingStatus}
						<LoaderCircle class="size-3.5 animate-spin" />
					{/if}
					{settingStatus ? "Saving…" : "Set status"}
				</Button>
			</form>
		</div>
	</section>

	<!-- Subscription + danger zone -->
	<section class="space-y-6">
		<div class="glass-card rounded-xl p-5">
			<h2 class="mb-3 text-sm font-semibold tracking-tight">Subscription</h2>
			{#await data.sub}
				<dl class="space-y-1.5 text-xs">
					{#each Array(3) as _, i (i)}
						<div class="flex justify-between gap-2">
							<Skeleton class="h-3 w-14" />
							<Skeleton class="h-3 w-20" />
						</div>
					{/each}
				</dl>
			{:then sub}
				{#if sub}
					<dl class="space-y-1.5 text-xs">
						<div class="flex justify-between gap-2"><dt class="text-muted-foreground">Plan</dt><dd class="font-medium">{sub.plan}</dd></div>
						<div class="flex justify-between gap-2"><dt class="text-muted-foreground">Status</dt><dd class="font-medium">{sub.status}</dd></div>
						{#if sub.currentPeriodEnd}
							<div class="flex justify-between gap-2"><dt class="text-muted-foreground">Renews</dt><dd class="font-medium">{new Date(sub.currentPeriodEnd).toLocaleDateString()}</dd></div>
						{/if}
						{#if sub.polarSubscriptionId}
							<div class="flex justify-between gap-2"><dt class="text-muted-foreground">Polar ID</dt><dd class="font-mono text-[10px]">{sub.polarSubscriptionId.slice(0, 12)}…</dd></div>
						{/if}
					</dl>
				{:else}
					<p class="text-xs text-muted-foreground">No subscription record.</p>
				{/if}
			{:catch}
				<InlineError message="Couldn't load subscription." />
			{/await}
		</div>

		<div class="rounded-xl border border-destructive/30 bg-destructive/4 p-5">
			<h2 class="mb-3 text-sm font-semibold tracking-tight text-destructive">Danger zone</h2>
			<div class="space-y-2">
				{#if t.banned}
					<form
						method="POST"
						action="?/unban"
						use:enhance={enhanceAction({
							setBusy: (b) => (unbanning = b),
							onSuccess: "User unbanned.",
							onFailure: false,
						})}
					>
						<Button variant="outline" size="sm" type="submit" disabled={unbanning} class="w-full gap-2">
							{#if unbanning}
								<LoaderCircle class="size-3.5 animate-spin" />
							{/if}
							{unbanning ? "Unbanning…" : "Unban user"}
						</Button>
					</form>
				{:else}
					<Button
						variant="outline"
						size="sm"
						class="w-full"
						onclick={() => (confirmBan = true)}
					>
						<ShieldOff class="size-3.5" /> Ban user
					</Button>
				{/if}
				<Button
					variant="outline"
					size="sm"
					class="w-full text-destructive hover:text-destructive"
					onclick={() => (confirmDelete = true)}
				>
					<Trash2 class="size-3.5" /> Delete user
				</Button>
			</div>
		</div>
	</section>
</div>

<!-- Sessions + password reset + audit, collapsed to reduce noise -->
<div class="mt-6 space-y-3">
	<Collapsible.Root class="glass-card rounded-xl">
		<Collapsible.Trigger class="flex w-full items-center justify-between gap-3 p-5 group/coll">
			<span class="flex items-center gap-2 text-sm font-semibold tracking-tight">
				<LogOut class="size-4 text-muted-foreground" />
				Sessions
				{#await data.sessions}
					(<Skeleton class="inline-block h-3 w-4 align-middle" />)
				{:then sessions}
					({sessions.length})
				{:catch}
					<!-- value hidden; the section below surfaces the error + retry -->
				{/await}
			</span>
			<ChevronDown class="size-4 text-muted-foreground transition-transform duration-200 group-data-[state=open]/coll:rotate-180" />
		</Collapsible.Trigger>
		<Collapsible.Content>
			<div class="border-t border-border/40 p-5">
				{#await data.sessions}
					<ul class="divide-y divide-border/30">
						{#each Array(3) as _, i (i)}
							<li class="flex items-center justify-between gap-3 py-2.5">
								<div class="min-w-0 flex-1 space-y-1.5">
									<Skeleton class="h-3 w-28" />
									<Skeleton class="h-2.5 w-48" />
								</div>
								<Skeleton class="h-6 w-14" />
							</li>
						{/each}
					</ul>
				{:then sessions}
					{#if sessions.length}
						<form
							method="POST"
							action="?/revokeAllSessions"
							class="mb-3"
							use:enhance={enhanceAction({
								setBusy: (b) => (revokingAll = b),
								onSuccess: "All sessions revoked.",
								onFailure: false,
							})}
						>
							<Button type="submit" size="sm" variant="outline" disabled={revokingAll} class="gap-2">
								{#if revokingAll}
									<LoaderCircle class="size-3.5 animate-spin" />
								{/if}
								{revokingAll ? "Revoking…" : "Revoke all sessions"}
							</Button>
						</form>
						<ul class="divide-y divide-border/30">
							{#each sessions as s (s.id)}
								<li class="flex items-center justify-between gap-3 py-2.5">
									<div class="min-w-0">
										<span class="block truncate font-mono text-[11px]">{s.ipAddress ?? "—"}</span>
										<span class="block truncate text-[11px] text-muted-foreground">{s.userAgent ?? "—"}</span>
									</div>
									<div class="flex items-center gap-2">
										{#if s.impersonatedBy}
											<Badge variant="outline" class="text-amber-600 dark:text-amber-400">impersonation</Badge>
										{/if}
										<form
											method="POST"
											action="?/revokeSession"
											use:enhance={enhanceAction({
												setBusy: (b) => (revokingSessionToken = b ? s.token : null),
												onSuccess: "Session revoked.",
												onFailure: false,
											})}
										>
											<input type="hidden" name="sessionToken" value={s.token} />
											<Button
												type="submit"
												variant="ghost"
												size="sm"
												disabled={revokingSessionToken === s.token}
												class="gap-1.5"
											>
												{#if revokingSessionToken === s.token}
													<LoaderCircle class="size-3.5 animate-spin" />
												{/if}
												{revokingSessionToken === s.token ? "Revoking…" : "Revoke"}
											</Button>
										</form>
									</div>
								</li>
							{/each}
						</ul>
					{:else}
						<p class="text-sm text-muted-foreground">No active sessions.</p>
					{/if}
				{:catch}
					<InlineError message="Couldn't load sessions." />
				{/await}
			</div>
		</Collapsible.Content>
	</Collapsible.Root>

	<Collapsible.Root class="glass-card rounded-xl">
		<Collapsible.Trigger class="flex w-full items-center justify-between gap-3 p-5 group/coll">
			<span class="flex items-center gap-2 text-sm font-semibold tracking-tight">
				<Key class="size-4 text-muted-foreground" />
				Set password (support reset)
			</span>
			<ChevronDown class="size-4 text-muted-foreground transition-transform duration-200 group-data-[state=open]/coll:rotate-180" />
		</Collapsible.Trigger>
		<Collapsible.Content>
			<form
				method="POST"
				action="?/setPassword"
				class="space-y-3 border-t border-border/40 p-5"
				use:enhance={enhanceAction({
					setBusy: (b) => (settingPassword = b),
					onSuccess: "Password set. Share securely with the user.",
					reset: () => (newPassword = ""),
				})}
			>
				<Label class="block">
					<span class="mb-1 block text-xs font-semibold text-foreground/85">New password</span>
					<Input
						type="text"
						name="password"
						bind:value={newPassword}
						placeholder="min 8 chars"
						class="h-9 font-mono"
					/>
				</Label>
				<Button type="submit" size="sm" disabled={settingPassword || !newPassword.trim()} class="gap-2">
					{#if settingPassword}
						<LoaderCircle class="size-3.5 animate-spin" />
					{/if}
					{settingPassword ? "Saving…" : "Set password"}
				</Button>
			</form>
		</Collapsible.Content>
	</Collapsible.Root>

	<Collapsible.Root class="glass-card rounded-xl">
		<Collapsible.Trigger class="flex w-full items-center justify-between gap-3 p-5 group/coll">
			<span class="flex items-center gap-2 text-sm font-semibold tracking-tight">
				<ClipboardList class="size-4 text-muted-foreground" />
				Audit log for this user
				{#await data.audit}
					(<Skeleton class="inline-block h-3 w-4 align-middle" />)
				{:then audit}
					({audit.length})
				{:catch}
					<!-- value hidden; the section below surfaces the error + retry -->
				{/await}
			</span>
			<ChevronDown class="size-4 text-muted-foreground transition-transform duration-200 group-data-[state=open]/coll:rotate-180" />
		</Collapsible.Trigger>
		<Collapsible.Content>
			<div class="border-t border-border/40 p-5">
				{#await data.audit}
					<ul class="divide-y divide-border/30">
						{#each Array(3) as _, i (i)}
							<li class="flex items-center justify-between gap-3 py-2">
								<Skeleton class="h-3 w-32" />
								<Skeleton class="h-3 w-24" />
							</li>
						{/each}
					</ul>
				{:then audit}
					{#if audit.length}
						<ul class="divide-y divide-border/30">
							{#each audit as a (a.id)}
								<li class="flex items-center justify-between gap-3 py-2">
									<span class="font-mono text-[11px] font-semibold uppercase tracking-wider">
										{a.action}
									</span>
									<span class="font-mono text-[10px] text-muted-foreground">
										{new Date(a.createdAt).toLocaleString()}
									</span>
								</li>
							{/each}
						</ul>
					{:else}
						<p class="text-sm text-muted-foreground">No audit entries yet.</p>
					{/if}
				{:catch}
					<InlineError message="Couldn't load the audit log." />
				{/await}
			</div>
		</Collapsible.Content>
	</Collapsible.Root>
</div>

<!-- Confirm delete dialog -->
<Dialog.Root bind:open={confirmDelete}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Delete this user?</Dialog.Title>
			<Dialog.Description>
				This removes <strong>{t.email}</strong> and all related rows (sessions,
				subscription, accounts). It can't be undone.
			</Dialog.Description>
		</Dialog.Header>
		<form
			method="POST"
			action="?/remove"
			use:enhance={() => {
				deleting = true;
				return async ({ result }) => {
					try {
						if (result.type === "redirect") {
							confirmDelete = false;
							toast.success("User deleted.");
							goto(result.location);
						} else if (result.type === "failure") {
							toast.error(String(result.data?.error));
						}
					} finally {
						deleting = false;
					}
				};
			}}
		>
			<Dialog.Footer>
				<Button type="button" variant="ghost" disabled={deleting} onclick={() => (confirmDelete = false)}>
					Cancel
				</Button>
				<Button type="submit" variant="destructive" disabled={deleting} class="gap-2">
					{#if deleting}
						<LoaderCircle class="size-3.5 animate-spin" />
					{/if}
					{deleting ? "Deleting…" : "Delete user"}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>

<!-- Confirm ban dialog -->
<Dialog.Root bind:open={confirmBan}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Ban {t.email}?</Dialog.Title>
			<Dialog.Description>
				The user's existing sessions will be revoked and they'll see the ban reason on next sign-in attempt.
			</Dialog.Description>
		</Dialog.Header>
		<form
			method="POST"
			action="?/ban"
			class="space-y-3"
			use:enhance={enhanceAction({
				setBusy: (b) => (banning = b),
				onSuccess: "User banned.",
				onFailure: false,
				invalidate: true,
				reset: () => (confirmBan = false),
			})}
		>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Reason</span>
				<Input bind:value={banReason} name="reason" placeholder="Optional, shown to user" class="h-9" />
			</Label>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Duration (days, blank = permanent)</span>
				<Input
					type="number"
					min="0"
					bind:value={banDays}
					name="expiresInDays"
					placeholder="e.g. 7"
					class="h-9"
				/>
			</Label>
			<Dialog.Footer>
				<Button type="button" variant="ghost" disabled={banning} onclick={() => (confirmBan = false)}>Cancel</Button>
				<Button type="submit" variant="destructive" disabled={banning} class="gap-2">
					{#if banning}
						<LoaderCircle class="size-3.5 animate-spin" />
					{/if}
					{banning ? "Banning…" : "Ban user"}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
