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

<header class="mb-8 flex flex-wrap items-end justify-between gap-4">
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
				use:enhance={() =>
					async ({ result }) => {
						if (result.type === "redirect") {
							toast.success("You've left the team.");
						}
					}}
			>
				<Button type="submit" variant="outline" size="sm">
					<LogOut class="size-3.5" /> Leave team
				</Button>
			</form>
		{/if}
	</div>
</header>

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
							<form
								method="POST"
								action="?/updateRole"
								use:enhance={() =>
									async ({ result, update }) => {
										if (result.type === "success") toast.success("Role updated.");
										await update();
									}}
							>
								<input type="hidden" name="memberId" value={m.id} />
								<Select.Root
									type="single"
									value={m.role}
									onValueChange={(v) => {
										const form = document.activeElement?.closest("form");
										if (!form) return;
										const input = document.createElement("input");
										input.type = "hidden";
										input.name = "role";
										input.value = String(v);
										form.appendChild(input);
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
								use:enhance={() =>
									async ({ result, update }) => {
										if (result.type === "success") toast.success("Member removed.");
										await update();
									}}
							>
								<input type="hidden" name="memberIdOrEmail" value={m.id} />
								<Button type="submit" variant="ghost" size="sm" class="text-destructive">
									<Trash2 class="size-3.5" />
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
						use:enhance={() =>
							async ({ result, update }) => {
								if (result.type === "success") {
									toast.success("Invitation sent.");
									inviteEmail = "";
								} else if (result.type === "failure") {
									toast.error(String(result.data?.error));
								}
								await update();
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
						<Button type="submit" size="sm" class="w-full gap-2">
							<Mail class="size-3.5" /> Send invite
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
									use:enhance={() =>
										async ({ result, update }) => {
											if (result.type === "success") toast.success("Invite canceled.");
											await update();
										}}
								>
									<input type="hidden" name="id" value={inv.id} />
									<Button type="submit" variant="ghost" size="sm">
										<X class="size-3.5" />
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
