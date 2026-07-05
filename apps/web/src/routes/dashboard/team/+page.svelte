<script lang="ts">
	import { enhance } from "$app/forms";
	import PageHeader from "$lib/dashboard/components/PageHeader.svelte";
	import SettingsSection from "$lib/dashboard/components/SettingsSection.svelte";
	import StatCard from "$lib/dashboard/components/StatCard.svelte";
	import { capitalize, initials, isManageable, seatsRemaining, seatsValue } from "$lib/dashboard/team.logic";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import * as Select from "@recast/ui/select";
	import { Skeleton } from "@recast/ui/skeleton";
	import { toast } from "@recast/ui/sonner";
	import {
		Building2,
		CalendarDays,
		Clock,
		Crown,
		Image,
		Link2,
		LoaderCircle,
		LogOut,
		Mail,
		ShieldCheck,
		Trash2,
		UserCog,
		UserPlus,
		Users,
	} from "@lucide/svelte";
	import { tick, untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let { data } = $props();

	let inviteEmail = $state("");
	let inviteRole = $state<"member" | "admin">("member");

	let leaving = $state(false);
	let savingProfile = $state(false);
	let settingDefault = $state(false);
	let inviting = $state(false);
	let cancellingInviteId = $state<string | null>(null);
	let removing = $state(false);
	let updatingRoleMemberId = $state<string | null>(null);

	let removeTarget = $state<{ id: string; name: string } | null>(null);
	let leaveOpen = $state(false);

	let pendingRole = $state<Record<string, string>>({});
	let roleForms = $state<Record<string, HTMLFormElement>>({});

	let teamName = $state(untrack(() => data.org.name));
	let teamSlug = $state(untrack(() => data.org.slug));
	let teamLogo = $state(untrack(() => data.org.logo ?? ""));

	const canManage = $derived(data.viewer.role === "owner" || data.viewer.role === "admin");
	const isOwner = $derived(data.viewer.role === "owner");
	const planLabel = $derived(capitalize(data.org.plan));
	const isDefaultWorkspace = $derived(data.viewer.defaultWorkspaceId === data.org.id);
	const createdLabel = $derived(
		new Date(data.org.createdAt).toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric",
		}),
	);
	const workspaceUrl = $derived(`/dashboard/recasts`);
	const roleLabel = $derived(capitalize(data.viewer.role));

	async function changeRole(memberId: string, value: string) {
		pendingRole = { ...pendingRole, [memberId]: value };
		await tick();
		roleForms[memberId]?.requestSubmit();
	}
</script>

<svelte:head>
	<title>Team - Recast Dashboard</title>
</svelte:head>

<div class="space-y-5" in:fly={{ y: 14, duration: 420, easing: cubicOut }}>
	<PageHeader icon={Users} title={data.org.name} subtitle="Manage workspace access, seats, invitations, and profile details.">
		<div class="flex w-full flex-wrap items-center gap-2 sm:w-auto">
			<Badge variant={data.org.plan === "free" ? "outline" : "secondary"}>
				{planLabel} plan
			</Badge>
			{#if isDefaultWorkspace}
				<Badge variant="secondary" class="gap-1">
					<ShieldCheck class="size-3" />
					Default
				</Badge>
			{:else}
				<form
					method="POST"
					action="?/setDefaultWorkspace"
					use:enhance={() => {
						settingDefault = true;
						return async ({ result, update }) => {
							try {
								if (result.type === "success") toast.success("Default workspace updated.");
								else if (result.type === "failure") toast.error(String(result.data?.error));
								await update({ reset: false });
							} finally {
								settingDefault = false;
							}
						};
					}}
				>
					<Button type="submit" variant="outline" size="sm" disabled={settingDefault} class="gap-2">
						{#if settingDefault}
							<LoaderCircle class="size-3.5 animate-spin" />
						{:else}
							<ShieldCheck class="size-3.5" />
						{/if}
						Make default
					</Button>
				</form>
			{/if}
			{#if !isOwner}
				<Button variant="outline" size="sm" onclick={() => (leaveOpen = true)} class="gap-2">
					<LogOut class="size-3.5" />
					Leave team
				</Button>
			{/if}
		</div>
	</PageHeader>

	<section class="grid grid-cols-1 gap-3 md:grid-cols-3 xl:grid-cols-4">
		{#await data.members}
			{#each Array(2) as _, i (i)}
				<Skeleton class="h-18 rounded-xl" />
			{/each}
		{:then members}
			{@const seatsLeft = seatsRemaining(data.caps.members, members.length)}
			<StatCard icon={Users} label="Members" value={seatsValue(data.caps.members, members.length)} />
			<StatCard
				icon={UserPlus}
				label="Seats left"
				value={Number.isFinite(seatsLeft) ? String(seatsLeft) : "Unlimited"}
			/>
		{/await}
		{#await data.invites then invites}
			<StatCard icon={Clock} label="Pending" value={String(invites.length)} />
		{/await}
		<StatCard icon={ShieldCheck} label="Your role" value={roleLabel} />
	</section>

	<section class="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
		<div class="space-y-4">
			<div class="glass-card rounded-xl p-5">
				<div class="flex flex-col gap-5 lg:flex-row lg:items-center lg:justify-between">
					<div class="flex min-w-0 items-center gap-4">
						<div class="grid size-16 shrink-0 place-items-center overflow-hidden rounded-2xl bg-foreground/6 text-foreground/70 ring-1 ring-border/40">
							{#if data.org.logo}
								<img src={data.org.logo} alt="" class="size-full object-cover" />
							{:else}
								<span class="text-lg font-bold">{initials(data.org.name)}</span>
							{/if}
						</div>
						<div class="min-w-0">
							<div class="flex flex-wrap items-center gap-2">
								<h2 class="truncate text-lg font-semibold tracking-tight text-foreground">
									{data.org.name}
								</h2>
								<Badge variant="outline" class="capitalize">{data.org.slug}</Badge>
							</div>
							<div class="mt-2 flex flex-wrap gap-3 text-xs text-muted-foreground">
								<span class="inline-flex items-center gap-1.5">
									<CalendarDays class="size-3.5" />
									Created {createdLabel}
								</span>
								<span class="inline-flex items-center gap-1.5">
									<Link2 class="size-3.5" />
									/team/{data.org.slug}
								</span>
							</div>
						</div>
					</div>
					<div class="flex shrink-0 flex-wrap gap-2">
						{#if isDefaultWorkspace}
							<Badge variant="secondary" class="gap-1">
								<ShieldCheck class="size-3" />
								Opens by default
							</Badge>
						{/if}
						<Button href={workspaceUrl} variant="outline" size="sm" class="gap-2">
							<Building2 class="size-3.5" />
							View workspace
						</Button>
					</div>
				</div>
			</div>

			{#if isOwner}
				<SettingsSection
					icon={Building2}
					title="Workspace profile"
					description="Name, slug, and logo shown across Recast."
				>
					<form
						method="POST"
						action="?/updateProfile"
						class="grid gap-4 md:grid-cols-[5rem_1fr]"
						use:enhance={() => {
							savingProfile = true;
							return async ({ result, update }) => {
								try {
									if (result.type === "success") toast.success("Team updated.");
									else if (result.type === "failure") toast.error(String(result.data?.error));
									await update({ reset: false });
								} finally {
									savingProfile = false;
								}
							};
						}}
					>
						<div class="flex justify-start">
							<div class="grid size-16 place-items-center overflow-hidden rounded-2xl bg-foreground/6 text-foreground/70 ring-1 ring-border/40">
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
									<Image class="size-5 opacity-50" />
								{/if}
							</div>
						</div>

						<div class="grid gap-4 sm:grid-cols-2">
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
							</Label>

							<Label class="block sm:col-span-2">
								<span class="mb-1 block text-xs font-semibold text-foreground/85">
									Logo URL <span class="font-normal text-muted-foreground">(optional)</span>
								</span>
								<Input
									bind:value={teamLogo}
									name="logo"
									type="url"
									placeholder="https://..."
									class="h-9"
								/>
							</Label>

							<div class="sm:col-span-2">
								<Button type="submit" size="sm" disabled={savingProfile} class="gap-2">
									{#if savingProfile}
										<LoaderCircle class="size-3.5 animate-spin" />
									{/if}
									{savingProfile ? "Saving..." : "Save changes"}
								</Button>
							</div>
						</div>
					</form>
				</SettingsSection>
			{/if}

			<SettingsSection icon={Users} title="Members" description="Roles and access for everyone in this workspace.">
				{#await data.members}
					<ul class="divide-y divide-border-low/40">
						{#each Array(4) as _, i (i)}
							<li class="flex items-center gap-3 py-3">
								<Skeleton class="size-9 shrink-0 rounded-full" />
								<div class="min-w-0 flex-1 space-y-1.5">
									<Skeleton class="h-3.5 w-32" />
									<Skeleton class="h-3 w-44" />
								</div>
								<Skeleton class="h-7 w-24" />
							</li>
						{/each}
					</ul>
				{:then members}
					<div class="overflow-x-auto">
						<table class="w-full text-sm">
							<thead>
								<tr class="border-b border-border-low/40 text-[10px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
									<th class="py-2 pr-4 text-left font-semibold">Member</th>
									<th class="px-4 py-2 text-left font-semibold">Role</th>
									<th class="py-2 pl-4 text-right font-semibold">Actions</th>
								</tr>
							</thead>
							<tbody>
								{#each members as m (m.id)}
									{@const you = m.userId === data.viewer.userId}
									<tr class="border-b border-border-low/25 last:border-0">
										<td class="max-w-0 py-3 pr-4">
											<div class="flex min-w-56 items-center gap-3">
												<span class="grid size-9 shrink-0 place-items-center rounded-full bg-foreground/6 text-[11px] font-bold text-foreground/70 ring-1 ring-border/40">
													{initials(m.name)}
												</span>
												<div class="min-w-0">
													<div class="flex items-center gap-1.5">
														<span class="truncate font-medium text-foreground">{m.name}</span>
														{#if you}
															<span class="rounded-full bg-foreground/8 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-muted-foreground">
																You
															</span>
														{/if}
													</div>
													<span class="block truncate text-xs text-muted-foreground">{m.email}</span>
												</div>
											</div>
										</td>
										<td class="px-4 py-3">
											{#if isManageable(m, data.viewer, canManage)}
												<form
													bind:this={roleForms[m.id]}
													method="POST"
													action="?/updateRole"
													use:enhance={() => {
														updatingRoleMemberId = m.id;
														return async ({ result, update }) => {
															try {
																if (result.type === "success") toast.success("Role updated.");
																else if (result.type === "failure") {
																	toast.error(String(result.data?.error) || "Couldn't update role.");
																	const { [m.id]: _drop, ...rest } = pendingRole;
																	pendingRole = rest;
																}
																await update({ reset: false });
															} finally {
																updatingRoleMemberId = null;
															}
														};
													}}
												>
													<input type="hidden" name="memberId" value={m.id} />
													<input type="hidden" name="role" value={pendingRole[m.id] ?? m.role} />
													<Select.Root
														type="single"
														value={pendingRole[m.id] ?? m.role}
														onValueChange={(v) => changeRole(m.id, String(v))}
													>
														<Select.Trigger class="h-8 w-28 text-xs capitalize">
															{capitalize(pendingRole[m.id] ?? m.role)}
														</Select.Trigger>
														<Select.Content>
															<Select.Item value="member">Member</Select.Item>
															<Select.Item value="admin">Admin</Select.Item>
														</Select.Content>
													</Select.Root>
												</form>
											{:else if m.role === "owner"}
												<Badge variant="secondary" class="gap-1"><Crown class="size-3" /> Owner</Badge>
											{:else if m.role === "admin"}
												<Badge variant="outline" class="gap-1"><ShieldCheck class="size-3" /> Admin</Badge>
											{:else}
												<Badge variant="outline">Member</Badge>
											{/if}
										</td>
										<td class="py-3 pl-4 text-right">
											{#if updatingRoleMemberId === m.id}
												<LoaderCircle class="mr-2 inline size-3.5 animate-spin text-muted-foreground" />
											{/if}
											{#if isManageable(m, data.viewer, canManage)}
												<Button
													variant="ghost"
													size="icon-sm"
													class="text-muted-foreground hover:text-destructive"
													aria-label="Remove {m.name}"
													onclick={() => (removeTarget = { id: m.id, name: m.name })}
												>
													<Trash2 class="size-3.5" />
												</Button>
											{:else}
												<span class="text-xs text-muted-foreground">Locked</span>
											{/if}
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{/await}
			</SettingsSection>
		</div>

		<aside class="space-y-4 xl:sticky xl:top-24 xl:self-start">
			{#if canManage}
				<SettingsSection icon={UserPlus} title="Invite teammate" description="Add someone by email.">
					{#await data.members}
						<div class="space-y-3">
							<Skeleton class="h-9 w-full" />
							<Skeleton class="h-9 w-full" />
							<Skeleton class="h-8 w-full" />
						</div>
					{:then members}
						{@const seatsLeft = seatsRemaining(data.caps.members, members.length)}
						{#if seatsLeft <= 0}
							<p class="rounded-lg border border-warning/30 bg-warning/8 p-3 text-xs text-muted-foreground">
								You're at the seat cap for the
								<span class="font-medium text-foreground">{planLabel}</span> plan.
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
										try {
											if (result.type === "success") {
												toast.success("Invitation sent.");
												inviteEmail = "";
											} else if (result.type === "failure") {
												toast.error(String(result.data?.error));
											}
											await update({ reset: false });
										} finally {
											inviting = false;
										}
									};
								}}
							>
								<Label class="block">
									<span class="mb-1 block text-xs font-semibold text-foreground/85">Email</span>
									<Input
										type="email"
										name="email"
										bind:value={inviteEmail}
										placeholder="teammate@company.com"
										required
										class="h-9"
									/>
								</Label>
								<Label class="block">
									<span class="mb-1 block text-xs font-semibold text-foreground/85">Role</span>
									<Select.Root type="single" bind:value={inviteRole} name="role">
										<Select.Trigger class="h-9 w-full capitalize">{capitalize(inviteRole)}</Select.Trigger>
										<Select.Content>
											<Select.Item value="member">Member</Select.Item>
											<Select.Item value="admin">Admin</Select.Item>
										</Select.Content>
									</Select.Root>
								</Label>
								<Button type="submit" size="sm" disabled={inviting || !inviteEmail.trim()} class="w-full gap-2">
									{#if inviting}
										<LoaderCircle class="size-3.5 animate-spin" />
									{:else}
										<Mail class="size-3.5" />
									{/if}
									{inviting ? "Sending..." : "Send invite"}
								</Button>
							</form>
						{/if}
					{/await}
				</SettingsSection>
			{/if}

			<SettingsSection icon={Clock} tone="muted" title="Pending invitations" description="Invites awaiting acceptance.">
				{#await data.invites}
					<ul class="divide-y divide-border-low/40">
						{#each Array(2) as _, i (i)}
							<li class="flex items-center justify-between gap-3 py-2">
								<div class="min-w-0 flex-1 space-y-1.5">
									<Skeleton class="h-3 w-36" />
									<Skeleton class="h-2.5 w-16" />
								</div>
								<Skeleton class="size-7 rounded-md" />
							</li>
						{/each}
					</ul>
				{:then invites}
					{#if invites.length}
						<ul class="divide-y divide-border-low/40">
							{#each invites as inv (inv.id)}
								<li class="flex items-center justify-between gap-3 py-2.5">
									<div class="min-w-0">
										<span class="block truncate text-xs font-medium">{inv.email}</span>
										<span class="block text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
											{capitalize(inv.role)}
										</span>
									</div>
									{#if canManage}
										<form
											method="POST"
											action="?/cancelInvite"
											use:enhance={() => {
												cancellingInviteId = inv.id;
												return async ({ result, update }) => {
													try {
														if (result.type === "success") toast.success("Invite canceled.");
														await update({ reset: false });
													} finally {
														cancellingInviteId = null;
													}
												};
											}}
										>
											<input type="hidden" name="id" value={inv.id} />
											<Button
												type="submit"
												variant="ghost"
												size="xs"
												disabled={cancellingInviteId === inv.id}
												class="text-muted-foreground hover:text-destructive"
											>
												{#if cancellingInviteId === inv.id}
													<LoaderCircle class="size-3.5 animate-spin" />
												{:else}
													Cancel
												{/if}
											</Button>
										</form>
									{/if}
								</li>
							{/each}
						</ul>
					{:else}
						<div class="flex flex-col items-center gap-2 py-6 text-center">
							<span class="glass-chip grid size-9 place-items-center rounded-lg text-muted-foreground">
								<Mail class="size-4" />
							</span>
							<p class="text-xs text-muted-foreground">No pending invitations.</p>
						</div>
					{/if}
				{/await}
			</SettingsSection>

			<SettingsSection icon={UserCog} tone="muted" title="Access model" description="How this workspace is governed.">
				<ul class="space-y-3 text-xs">
					<li class="flex items-start gap-2">
						<ShieldCheck class="mt-0.5 size-3.5 text-primary" />
						<span class="text-muted-foreground">
							Owners manage profile details and destructive actions.
						</span>
					</li>
					<li class="flex items-start gap-2">
						<UserPlus class="mt-0.5 size-3.5 text-primary" />
						<span class="text-muted-foreground">
							Admins can invite teammates and manage member roles.
						</span>
					</li>
					<li class="flex items-start gap-2">
						<Users class="mt-0.5 size-3.5 text-primary" />
						<span class="text-muted-foreground">
							Members can access workspace recasts and shared assets.
						</span>
					</li>
				</ul>
			</SettingsSection>
		</aside>
	</section>
</div>

<Dialog.Root
	open={removeTarget !== null}
	onOpenChange={(o) => {
		if (!o) removeTarget = null;
	}}
>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Remove {removeTarget?.name}?</Dialog.Title>
			<Dialog.Description>
				They'll immediately lose access to this team's recasts. You can invite them again later.
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" size="sm" onclick={() => (removeTarget = null)}>Cancel</Button>
			<form
				method="POST"
				action="?/removeMember"
				use:enhance={() => {
					removing = true;
					return async ({ result, update }) => {
						try {
							if (result.type === "success") {
								toast.success("Member removed.");
								removeTarget = null;
							} else if (result.type === "failure") {
								toast.error(String(result.data?.error) || "Couldn't remove member.");
							}
							await update({ reset: false });
						} finally {
							removing = false;
						}
					};
				}}
			>
				<input type="hidden" name="memberIdOrEmail" value={removeTarget?.id} />
				<Button type="submit" variant="destructive" size="sm" disabled={removing} class="gap-2">
					{#if removing}
						<LoaderCircle class="size-3.5 animate-spin" />
					{:else}
						<Trash2 class="size-3.5" />
					{/if}
					Remove
				</Button>
			</form>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={leaveOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Leave {data.org.name}?</Dialog.Title>
			<Dialog.Description>
				You'll lose access to this team's recasts. An owner or admin would need to re-invite you.
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" size="sm" onclick={() => (leaveOpen = false)}>Cancel</Button>
			<form
				method="POST"
				action="?/leave"
				use:enhance={() => {
					leaving = true;
					return async ({ result }) => {
						try {
							if (result.type === "redirect") toast.success("You've left the team.");
						} finally {
							leaving = false;
						}
					};
				}}
			>
				<Button type="submit" variant="destructive" size="sm" disabled={leaving} class="gap-2">
					{#if leaving}
						<LoaderCircle class="size-3.5 animate-spin" />
					{:else}
						<LogOut class="size-3.5" />
					{/if}
					Leave team
				</Button>
			</form>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
