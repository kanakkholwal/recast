<script lang="ts">
	import { enhance } from "$app/forms";
	import { goto, invalidateAll } from "$app/navigation";
	import { untrack } from "svelte";
	import { authClient } from "$lib/auth/client";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import * as Collapsible from "@recast/ui/collapsible";
	import * as Dialog from "@recast/ui/dialog";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import * as Select from "@recast/ui/select";
	import { toast } from "@recast/ui/sonner";
	import {
		ArrowLeft,
		ChevronDown,
		ClipboardList,
		Crown,
		Key,
		LogOut,
		ShieldOff,
		Trash2,
		UserCog,
	} from "@lucide/svelte";

	let { data } = $props();

	const t = $derived(data.target);

	// Seed editable form state from the initial server load so it doesn't
	// reset itself when a form action returns and re-runs the load.
	let role = $state(untrack(() => data.target.role ?? "user"));
	let status = $state(untrack(() => data.target.status ?? "active"));
	let name = $state(untrack(() => data.target.name ?? ""));

	let banReason = $state("");
	let banDays = $state("");
	let newPassword = $state("");

	let confirmDelete = $state(false);
	let confirmBan = $state(false);

	async function impersonate() {
		const { error } = await authClient.admin.impersonateUser({ userId: t.id });
		if (error) {
			toast.error(error.message ?? "Couldn't start impersonation.");
			return;
		}
		// Cookie has been swapped to the impersonation session — leave admin
		// and land in the impersonated user's dashboard.
		window.location.href = "/dashboard";
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
		<Button variant="outline" size="sm" onclick={impersonate}>
			<UserCog class="size-3.5" />
			Impersonate
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
			use:enhance={() =>
				async ({ result, update }) => {
					if (result.type === "success") toast.success("Profile updated.");
					else if (result.type === "failure") toast.error(String(result.data?.error));
					await update();
				}}
		>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Name</span>
				<Input bind:value={name} name="name" class="h-9" />
			</Label>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Email</span>
				<Input value={t.email} readonly disabled class="h-9 font-mono" />
				<span class="mt-1 block text-[11px] text-muted-foreground">
					Email changes go through the user's own settings — admins can't edit it.
				</span>
			</Label>
			<Button type="submit" size="sm">Save profile</Button>
		</form>

		<hr class="my-6 border-border/40" />

		<div class="grid gap-4 sm:grid-cols-2">
			<form
				method="POST"
				action="?/setRole"
				use:enhance={() =>
					async ({ result, update }) => {
						if (result.type === "success") toast.success("Role updated.");
						else if (result.type === "failure") toast.error(String(result.data?.error));
						await update();
					}}
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
				<Button type="submit" size="sm" class="mt-2 w-full">Set role</Button>
			</form>

			<form
				method="POST"
				action="?/setStatus"
				use:enhance={() =>
					async ({ result, update }) => {
						if (result.type === "success") toast.success("Status updated.");
						else if (result.type === "failure") toast.error(String(result.data?.error));
						await update();
					}}
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
				<Button type="submit" size="sm" class="mt-2 w-full">Set status</Button>
			</form>
		</div>
	</section>

	<!-- Subscription + danger zone -->
	<section class="space-y-6">
		<div class="glass-card rounded-xl p-5">
			<h2 class="mb-3 text-sm font-semibold tracking-tight">Subscription</h2>
			{#if data.sub}
				<dl class="space-y-1.5 text-xs">
					<div class="flex justify-between gap-2"><dt class="text-muted-foreground">Plan</dt><dd class="font-medium">{data.sub.plan}</dd></div>
					<div class="flex justify-between gap-2"><dt class="text-muted-foreground">Status</dt><dd class="font-medium">{data.sub.status}</dd></div>
					{#if data.sub.currentPeriodEnd}
						<div class="flex justify-between gap-2"><dt class="text-muted-foreground">Renews</dt><dd class="font-medium">{new Date(data.sub.currentPeriodEnd).toLocaleDateString()}</dd></div>
					{/if}
					{#if data.sub.polarSubscriptionId}
						<div class="flex justify-between gap-2"><dt class="text-muted-foreground">Polar ID</dt><dd class="font-mono text-[10px]">{data.sub.polarSubscriptionId.slice(0, 12)}…</dd></div>
					{/if}
				</dl>
			{:else}
				<p class="text-xs text-muted-foreground">No subscription record.</p>
			{/if}
		</div>

		<div class="rounded-xl border border-destructive/30 bg-destructive/4 p-5">
			<h2 class="mb-3 text-sm font-semibold tracking-tight text-destructive">Danger zone</h2>
			<div class="space-y-2">
				{#if t.banned}
					<form
						method="POST"
						action="?/unban"
						use:enhance={() =>
							async ({ result, update }) => {
								if (result.type === "success") toast.success("User unbanned.");
								await update();
							}}
					>
						<Button variant="outline" size="sm" type="submit" class="w-full">
							Unban user
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
				Sessions ({data.sessions.length})
			</span>
			<ChevronDown class="size-4 text-muted-foreground transition-transform duration-200 group-data-[state=open]/coll:rotate-180" />
		</Collapsible.Trigger>
		<Collapsible.Content>
			<div class="border-t border-border/40 p-5">
				{#if data.sessions.length}
					<form
						method="POST"
						action="?/revokeAllSessions"
						class="mb-3"
						use:enhance={() =>
							async ({ result, update }) => {
								if (result.type === "success") toast.success("All sessions revoked.");
								await update();
							}}
					>
						<Button type="submit" size="sm" variant="outline">Revoke all sessions</Button>
					</form>
					<ul class="divide-y divide-border/30">
						{#each data.sessions as s (s.id)}
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
										use:enhance={() =>
											async ({ result, update }) => {
												if (result.type === "success") toast.success("Session revoked.");
												await update();
											}}
									>
										<input type="hidden" name="sessionToken" value={s.token} />
										<Button type="submit" variant="ghost" size="sm">Revoke</Button>
									</form>
								</div>
							</li>
						{/each}
					</ul>
				{:else}
					<p class="text-sm text-muted-foreground">No active sessions.</p>
				{/if}
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
				use:enhance={() =>
					async ({ result, update }) => {
						if (result.type === "success") {
							toast.success("Password set. Share securely with the user.");
							newPassword = "";
						} else if (result.type === "failure") {
							toast.error(String(result.data?.error));
						}
						await update();
					}}
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
				<Button type="submit" size="sm">Set password</Button>
			</form>
		</Collapsible.Content>
	</Collapsible.Root>

	<Collapsible.Root class="glass-card rounded-xl">
		<Collapsible.Trigger class="flex w-full items-center justify-between gap-3 p-5 group/coll">
			<span class="flex items-center gap-2 text-sm font-semibold tracking-tight">
				<ClipboardList class="size-4 text-muted-foreground" />
				Audit log for this user ({data.audit.length})
			</span>
			<ChevronDown class="size-4 text-muted-foreground transition-transform duration-200 group-data-[state=open]/coll:rotate-180" />
		</Collapsible.Trigger>
		<Collapsible.Content>
			<div class="border-t border-border/40 p-5">
				{#if data.audit.length}
					<ul class="divide-y divide-border/30">
						{#each data.audit as a (a.id)}
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
			use:enhance={() =>
				async ({ result }) => {
					if (result.type === "redirect") {
						confirmDelete = false;
						toast.success("User deleted.");
						goto(result.location);
					} else if (result.type === "failure") {
						toast.error(String(result.data?.error));
					}
				}}
		>
			<Dialog.Footer>
				<Button type="button" variant="ghost" onclick={() => (confirmDelete = false)}>
					Cancel
				</Button>
				<Button type="submit" variant="destructive">Delete user</Button>
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
			use:enhance={() =>
				async ({ result, update }) => {
					if (result.type === "success") {
						confirmBan = false;
						toast.success("User banned.");
						await invalidateAll();
					}
					await update();
				}}
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
				<Button type="button" variant="ghost" onclick={() => (confirmBan = false)}>Cancel</Button>
				<Button type="submit" variant="destructive">Ban user</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
