<script lang="ts">
	import { enhance } from "$app/forms";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import * as Select from "@recast/ui/select";
	import { toast } from "@recast/ui/sonner";
	import {
		Crown,
		Image,
		LoaderCircle,
		LogOut,
		Mail,
		Send,
		ShieldCheck,
		Trash2,
		UserCog,
		Wrench,
		X,
	} from "@lucide/svelte";
	import { untrack } from "svelte";

	let { data } = $props();

	let inviteEmail = $state("");
	let inviteRole = $state<"member" | "admin">("member");

	// Per-action in-flight tracking so each button can show its own spinner
	// without disabling the entire page.
	let leaving = $state(false);
	let savingProfile = $state(false);
	let inviting = $state(false);
	let cancellingInviteId = $state<string | null>(null);
	let removingMemberId = $state<string | null>(null);
	let updatingRoleMemberId = $state<string | null>(null);

	// Owner-editable profile fields. Seeded once from server data so the
	// inputs don't snap back during form submission.
	let teamName = $state(untrack(() => data.org.name));
	let teamSlug = $state(untrack(() => data.org.slug));
	let teamLogo = $state(untrack(() => data.org.logo ?? ""));

	const canManage = $derived(
		data.viewer.role === "owner" || data.viewer.role === "admin",
	);
	const isOwner = $derived(data.viewer.role === "owner");

	const seatsRemaining = $derived(
		Number.isFinite(data.caps.members)
			? Math.max(0, data.caps.members - data.members.length)
			: Number.POSITIVE_INFINITY,
	);
	const seatsLabel = $derived(
		Number.isFinite(data.caps.members)
			? `${data.members.length} / ${data.caps.members} seats used`
			: `${data.members.length} seats used · no cap`,
	);
</script>

<header class="mb-8 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
	<div class="min-w-0">
		<h1 class="truncate text-2xl font-semibold tracking-tight">{data.org.name}</h1>
		<p class="mt-1 text-sm text-muted-foreground">
			{seatsLabel}
		</p>
	</div>
	<div class="flex items-center gap-2">
		<Badge variant={data.org.plan === "free" ? "outline" : "secondary"}>
			{data.org.plan}
		</Badge>
		{#if !isOwner}
			<form
				method="POST"
				action="?/leave"
				use:enhance={() => {
					leaving = true;
					return async ({ result }) => {
						if (result.type === "redirect") {
							toast.success("You've left the team.");
						}
						leaving = false;
					};
				}}
			>
				<Button type="submit" variant="outline" size="sm" disabled={leaving}>
					{#if leaving}
						<LoaderCircle class="size-3.5 animate-spin" />
					{:else}
						<LogOut class="size-3.5" />
					{/if}
					{leaving ? "Leaving…" : "Leave team"}
				</Button>
			</form>
		{/if}
	</div>
</header>

{#if isOwner}
	<section class="glass-card mb-6 rounded-xl p-5">
		<header class="mb-4 flex items-center gap-2.5">
			<span class="glass-chip grid size-8 place-items-center rounded-lg text-primary">
				<Wrench class="size-4" />
			</span>
			<div>
				<h2 class="text-sm font-semibold tracking-tight">Team profile</h2>
				<p class="text-[11px] text-muted-foreground">
					Only the owner sees this. Plan changes happen in admin tooling.
				</p>
			</div>
		</header>

		<form
			method="POST"
			action="?/updateProfile"
			class="grid gap-3 sm:grid-cols-[88px_1fr]"
			use:enhance={() => {
				savingProfile = true;
				return async ({ result, update }) => {
					if (result.type === "success") toast.success("Team updated.");
					else if (result.type === "failure")
						toast.error(String(result.data?.error));
					await update();
					savingProfile = false;
				};
			}}
		>
			<div class="row-span-3 flex justify-center sm:justify-start">
				<div class="relative grid size-20 place-items-center overflow-hidden rounded-2xl bg-foreground/6 text-foreground/70 ring-1 ring-border/40">
					{#if teamLogo}
						<img
							src={teamLogo}
							alt="Team logo preview"
							class="size-full object-cover"
							onerror={(e) => {
								(e.currentTarget as HTMLImageElement).style.display = "none";
							}}
						/>
					{:else}
						<Image class="size-6 opacity-50" />
					{/if}
				</div>
			</div>

			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Name</span>
				<Input bind:value={teamName} name="name" class="h-9" required />
			</Label>

			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Slug</span>
				<Input
					bind:value={teamSlug}
					name="slug"
					class="h-9 font-mono lowercase"
					pattern="[a-z0-9][a-z0-9-]+[a-z0-9]"
					required
				/>
				<span class="mt-1 block text-[10px] text-muted-foreground">
					Used in URLs. Lowercase letters, numbers, hyphens. Must be unique.
				</span>
			</Label>

			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">
					Logo URL <span class="font-normal text-muted-foreground">(optional)</span>
				</span>
				<Input
					bind:value={teamLogo}
					name="logo"
					type="url"
					placeholder="https://…"
					class="h-9"
				/>
			</Label>

			<div class="sm:col-start-2">
				<Button type="submit" size="sm" disabled={savingProfile} class="gap-2">
					{#if savingProfile}
						<LoaderCircle class="size-3.5 animate-spin" />
					{/if}
					{savingProfile ? "Saving…" : "Save changes"}
				</Button>
			</div>
		</form>
	</section>
{/if}

<div class="grid gap-6 lg:grid-cols-3">
	<!-- Members -->
	<section class="glass-card rounded-xl p-5 lg:col-span-2">
		<h2 class="mb-4 text-sm font-semibold tracking-tight">Members</h2>
		<ul class="divide-y divide-border/30">
			{#each data.members as m (m.id)}
				<li class="flex flex-wrap items-center justify-between gap-3 py-3">
					<div class="min-w-0">
						<span class="block truncate text-sm font-medium">{m.name}</span>
						<span class="block truncate text-xs text-muted-foreground">{m.email}</span>
					</div>
					<div class="flex items-center gap-2">
						{#if m.role === "owner"}
							<Badge variant="secondary" class="gap-1"><Crown class="size-3" /> owner</Badge>
						{:else if m.role === "admin"}
							<Badge variant="outline" class="gap-1"><ShieldCheck class="size-3" /> admin</Badge>
						{:else}
							<Badge variant="outline">member</Badge>
						{/if}
						{#if canManage && m.userId !== data.viewer.userId && m.role !== "owner"}
							{#if updatingRoleMemberId === m.id}
								<LoaderCircle class="size-3.5 animate-spin text-muted-foreground" />
							{/if}
							<form
								method="POST"
								action="?/updateRole"
								use:enhance={() => {
									updatingRoleMemberId = m.id;
									return async ({ result, update }) => {
										if (result.type === "success") toast.success("Role updated.");
										await update();
										updatingRoleMemberId = null;
									};
								}}
							>
								<input type="hidden" name="memberId" value={m.id} />
								<Select.Root
									type="single"
									value={m.role}
									onValueChange={(v) => {
										const form = document.activeElement?.closest("form");
										if (!form) return;
										// Reuse the existing hidden input if there is one, append
										// a fresh one otherwise. Two role inputs would serialize
										// the stale first value because `FormData.get` returns the
										// earliest entry — which is exactly what the user just
										// changed away from.
										let input = form.querySelector<HTMLInputElement>(
											'input[type="hidden"][name="role"]',
										);
										if (!input) {
											input = document.createElement("input");
											input.type = "hidden";
											input.name = "role";
											form.appendChild(input);
										}
										input.value = String(v);
										(form as HTMLFormElement).requestSubmit();
									}}
								>
									<Select.Trigger class="h-7 w-24 text-xs">{m.role}</Select.Trigger>
									<Select.Content>
										<Select.Item value="member">member</Select.Item>
										<Select.Item value="admin">admin</Select.Item>
									</Select.Content>
								</Select.Root>
							</form>
							<form
								method="POST"
								action="?/removeMember"
								use:enhance={() => {
									removingMemberId = m.id;
									return async ({ result, update }) => {
										if (result.type === "success") toast.success("Member removed.");
										await update();
										removingMemberId = null;
									};
								}}
							>
								<input type="hidden" name="memberIdOrEmail" value={m.id} />
								<Button
									type="submit"
									variant="ghost"
									size="sm"
									disabled={removingMemberId === m.id}
									class="text-destructive"
								>
									{#if removingMemberId === m.id}
										<LoaderCircle class="size-3.5 animate-spin" />
									{:else}
										<Trash2 class="size-3.5" />
									{/if}
								</Button>
							</form>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	</section>

	<!-- Invite + pending invitations -->
	<section class="space-y-6">
		{#if canManage}
			<div class="glass-card rounded-xl p-5">
				<h2 class="mb-3 flex items-center gap-2 text-sm font-semibold tracking-tight">
					<Send class="size-4 text-muted-foreground" />
					Invite a teammate
				</h2>
				{#if seatsRemaining <= 0}
					<p class="rounded-lg border border-amber-500/30 bg-amber-500/8 p-3 text-xs text-muted-foreground">
						You're at the seat cap for the
						<span class="font-medium text-foreground">{data.org.plan}</span> plan.
						{#if data.org.plan === "free"}
							<a href="/pricing" class="font-semibold text-foreground hover:text-primary">Upgrade to Pro</a>
							for 50 seats.
						{/if}
					</p>
				{:else}
					<form
						method="POST"
						action="?/invite"
						class="space-y-3"
						use:enhance={() => {
							inviting = true;
							return async ({ result, update }) => {
								if (result.type === "success") {
									toast.success("Invitation sent.");
									inviteEmail = "";
								} else if (result.type === "failure") {
									toast.error(String(result.data?.error));
								}
								await update();
								inviting = false;
							};
						}}
					>
						<Label class="block">
							<span class="mb-1 block text-xs font-semibold text-foreground/85">Email</span>
							<Input
								type="email"
								name="email"
								bind:value={inviteEmail}
								placeholder="teammate@startup.com"
								required
								class="h-9"
							/>
						</Label>
						<Label class="block">
							<span class="mb-1 block text-xs font-semibold text-foreground/85">Role</span>
							<Select.Root type="single" bind:value={inviteRole} name="role">
								<Select.Trigger class="h-9 w-full">{inviteRole}</Select.Trigger>
								<Select.Content>
									<Select.Item value="member">member</Select.Item>
									<Select.Item value="admin">admin</Select.Item>
								</Select.Content>
							</Select.Root>
						</Label>
						<Button type="submit" size="sm" disabled={inviting || !inviteEmail.trim()} class="w-full gap-2">
							{#if inviting}
								<LoaderCircle class="size-3.5 animate-spin" />
							{:else}
								<Mail class="size-3.5" />
							{/if}
							{inviting ? "Sending invite…" : "Send invite"}
						</Button>
					</form>
				{/if}
			</div>
		{/if}

		<div class="glass-card rounded-xl p-5">
			<h2 class="mb-3 text-sm font-semibold tracking-tight">
				Pending invitations
			</h2>
			{#if data.invites.length}
				<ul class="divide-y divide-border/30">
					{#each data.invites as inv (inv.id)}
						<li class="flex items-center justify-between gap-3 py-2">
							<div class="min-w-0">
								<span class="block truncate text-xs font-mono">{inv.email}</span>
								<span class="block text-[10px] uppercase tracking-wider text-muted-foreground">
									{inv.role}
								</span>
							</div>
							{#if canManage}
								<form
									method="POST"
									action="?/cancelInvite"
									use:enhance={() => {
										cancellingInviteId = inv.id;
										return async ({ result, update }) => {
											if (result.type === "success") toast.success("Invite canceled.");
											await update();
											cancellingInviteId = null;
										};
									}}
								>
									<input type="hidden" name="id" value={inv.id} />
									<Button
										type="submit"
										variant="ghost"
										size="sm"
										disabled={cancellingInviteId === inv.id}
									>
										{#if cancellingInviteId === inv.id}
											<LoaderCircle class="size-3.5 animate-spin" />
										{:else}
											<X class="size-3.5" />
										{/if}
									</Button>
								</form>
							{/if}
						</li>
					{/each}
				</ul>
			{:else}
				<p class="text-xs text-muted-foreground">No pending invitations.</p>
			{/if}
		</div>
	</section>
</div>
