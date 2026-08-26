<script lang="ts">
import {
	ChevronLeft,
	ChevronRight,
	Crown,
	LoaderCircle,
	Search,
	ShieldOff,
	UserPlus,
	X,
} from "@recast/icons";
import { Badge } from "@recast/ui/badge";
import { Button } from "@recast/ui/button";
import * as Dialog from "@recast/ui/dialog";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import * as Select from "@recast/ui/select";
import { Skeleton } from "@recast/ui/skeleton";
import { cn } from "@recast/ui/utils";
import { untrack } from "svelte";
import { enhance } from "$app/forms";
import { goto } from "$app/navigation";
import { page } from "$app/state";
import InlineError from "$lib/components/InlineError.svelte";
import { enhanceAction } from "$lib/forms/enhance";
import { isValidEmail } from "$lib/validation/email";
import {
	ariaSort,
	buildPageQuery,
	buildSortQuery,
	buildUsersQuery,
	sortIndicator,
} from "./users-filters.logic";

let { data } = $props();

// Seed editable form state once from the URL-driven `data.filters` — we
// don't want a later page navigation to clobber what the user just typed.
let q = $state(untrack(() => data.filters.q));
let searchField = $state<"email" | "name">(untrack(() => data.filters.field));
let roleFilter = $state<string>(untrack(() => data.filters.role ?? "all"));
let statusFilter = $state<string>(untrack(() => data.filters.status ?? "all"));

// Human labels for the filter controls — the raw enum values ("all",
// "pending") read as unfinished in the UI.
const FIELD_LABEL: Record<string, string> = { email: "Email", name: "Name" };
const ROLE_LABEL: Record<string, string> = { all: "All roles", user: "Users", admin: "Admins" };
const STATUS_LABEL: Record<string, string> = {
	all: "All statuses",
	active: "Active",
	pending: "Waitlist",
};
const hasActiveFilters = $derived(
	q.trim() !== "" || roleFilter !== "all" || statusFilter !== "all",
);

let inviteOpen = $state(false);
let inviting = $state(false);
let inviteEmail = $state("");
let inviteName = $state("");
const canInvite = $derived(isValidEmail(inviteEmail));

function resetInvite() {
	inviteOpen = false;
	inviteEmail = "";
	inviteName = "";
}

function applyFilters(reset = true) {
	goto(
		buildUsersQuery(
			{
				q,
				field: searchField,
				role: roleFilter,
				status: statusFilter,
				sort: data.filters.sort,
				dir: data.filters.dir,
			},
			{ limit: data.limit, offset: data.offset, reset },
		),
		{ keepFocus: true },
	);
}

// Live search — debounced so we don't navigate on every keystroke. Enter
// (form submit) applies immediately and cancels the pending debounce.
let searchTimer: ReturnType<typeof setTimeout> | undefined;
function debouncedSearch() {
	clearTimeout(searchTimer);
	searchTimer = setTimeout(() => applyFilters(), 350);
}
function submitSearch(e: SubmitEvent) {
	e.preventDefault();
	clearTimeout(searchTimer);
	applyFilters();
}

// Discrete controls apply on change — no separate "Apply" step.
function selectField(v: string) {
	searchField = v as "email" | "name";
	if (q.trim()) applyFilters();
}
function selectRole(v: string) {
	roleFilter = v;
	applyFilters();
}
function selectStatus(v: string) {
	statusFilter = v;
	applyFilters();
}
function clearFilters() {
	clearTimeout(searchTimer);
	q = "";
	searchField = "email";
	roleFilter = "all";
	statusFilter = "all";
	applyFilters();
}

function changePage(delta: number) {
	goto(
		buildPageQuery({
			search: page.url.searchParams.toString(),
			offset: data.offset,
			limit: data.limit,
			delta,
		}),
	);
}

function toggleSort(field: string) {
	goto(
		buildSortQuery({
			search: page.url.searchParams.toString(),
			currentSort: data.filters.sort,
			currentDir: data.filters.dir,
			field,
		}),
	);
}
</script>

<header class="mb-6 flex flex-wrap items-end justify-between gap-3">
	<div>
		<h1 class="text-2xl font-semibold tracking-tight">Users</h1>
		<p class="mt-1 text-sm text-muted-foreground">
			{#await data.list}
				Loading…
			{:then list}
				{@const startIdx = list.total === 0 ? 0 : data.offset + 1}
				{@const endIdx = Math.min(data.offset + data.limit, list.total)}
				{list.total.toLocaleString()} total · showing {startIdx}–{endIdx}
			{:catch}
				<!-- value hidden; the section below surfaces the error + retry -->
			{/await}
		</p>
	</div>
	<Button variant="default" class="gap-2" onclick={() => (inviteOpen = true)}>
		<UserPlus class="size-4" />
		Invite user
	</Button>
</header>

<Dialog.Root bind:open={inviteOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Invite a user</Dialog.Title>
			<Dialog.Description>
				Creates an active account and emails them a link to set their password. Skips the
				waitlist. If they're already on the waitlist, this approves them.
			</Dialog.Description>
		</Dialog.Header>
		<form
			method="POST"
			action="?/invite"
			class="space-y-3"
			use:enhance={enhanceAction({
				setBusy: (b) => (inviting = b),
				onSuccess: "Invite sent.",
				invalidate: true,
				reset: resetInvite,
			})}
		>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Email</span>
				<Input
					type="email"
					name="email"
					bind:value={inviteEmail}
					placeholder="name@company.com"
					autocomplete="off"
					required
					class="h-9"
				/>
			</Label>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">
					Name <span class="font-normal text-muted-foreground">(optional)</span>
				</span>
				<Input
					name="name"
					bind:value={inviteName}
					placeholder="Defaults to the part before the @"
					autocomplete="off"
					class="h-9"
				/>
			</Label>
			<Dialog.Footer>
				<Button type="button" variant="ghost" disabled={inviting} onclick={resetInvite}>
					Cancel
				</Button>
				<Button type="submit" disabled={inviting || !canInvite} class="gap-2">
					{#if inviting}
						<LoaderCircle class="size-3.5 animate-spin" />
					{/if}
					{inviting ? "Sending…" : "Send invite"}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>

<form
	class="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-border/40 bg-card/30 p-2"
	onsubmit={submitSearch}
>
	<div class="relative min-w-56 flex-1">
		<Search class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
		<Input
			type="search"
			placeholder="Search users by {FIELD_LABEL[searchField].toLowerCase()}…"
			bind:value={q}
			oninput={debouncedSearch}
			aria-label="Search users"
			class="h-9 pl-9"
		/>
	</div>

	<Select.Root type="single" value={searchField} onValueChange={selectField}>
		<Select.Trigger class="h-9 w-28" aria-label="Search field">
			{FIELD_LABEL[searchField]}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="email">Email</Select.Item>
			<Select.Item value="name">Name</Select.Item>
		</Select.Content>
	</Select.Root>

	<Select.Root type="single" value={roleFilter} onValueChange={selectRole}>
		<Select.Trigger class="h-9 w-36" aria-label="Filter by role">
			{ROLE_LABEL[roleFilter]}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="all">All roles</Select.Item>
			<Select.Item value="user">Users</Select.Item>
			<Select.Item value="admin">Admins</Select.Item>
		</Select.Content>
	</Select.Root>

	<Select.Root type="single" value={statusFilter} onValueChange={selectStatus}>
		<Select.Trigger class="h-9 w-40" aria-label="Filter by status">
			{STATUS_LABEL[statusFilter]}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="all">All statuses</Select.Item>
			<Select.Item value="active">Active</Select.Item>
			<Select.Item value="pending">Waitlist</Select.Item>
		</Select.Content>
	</Select.Root>

	{#if hasActiveFilters}
		<Button type="button" size="sm" variant="ghost" class="gap-1.5 text-muted-foreground" onclick={clearFilters}>
			<X class="size-3.5" /> Clear
		</Button>
	{/if}
</form>

<!-- Desktop: semantic sortable table. Hidden below lg where it would force
     horizontal scroll; the card grid below takes over there. -->
<div class="hidden overflow-hidden rounded-xl glass-card lg:block">
	<div class="overflow-x-auto">
		<table class="w-full min-w-160 text-left text-sm">
			<thead class="border-b border-border/40 bg-foreground/2 text-[11px] uppercase tracking-[0.12em] text-muted-foreground">
				<tr>
					<th class="px-4 py-2.5" aria-sort={ariaSort(data.filters.sort, data.filters.dir, "name")}>
						<button class="inline-flex items-center gap-1 font-semibold transition-colors hover:text-foreground" onclick={() => toggleSort("name")}>
							User <span class="text-primary">{sortIndicator(data.filters.sort, data.filters.dir, "name")}</span>
						</button>
					</th>
					<th class="px-4 py-2.5">Role / Status</th>
					<th class="px-4 py-2.5" aria-sort={ariaSort(data.filters.sort, data.filters.dir, "createdAt")}>
						<button class="inline-flex items-center gap-1 font-semibold transition-colors hover:text-foreground" onclick={() => toggleSort("createdAt")}>
							Joined <span class="text-primary">{sortIndicator(data.filters.sort, data.filters.dir, "createdAt")}</span>
						</button>
					</th>
					<th class="px-4 py-2.5 text-right">Actions</th>
				</tr>
			</thead>
			<tbody class="divide-y divide-border/30">
				{#await data.list}
					{#each Array(8) as _, i (i)}
						<tr>
							<td class="px-4 py-3">
								<div class="space-y-1.5">
									<Skeleton class="h-3.5 w-32" />
									<Skeleton class="h-3 w-44" />
								</div>
							</td>
							<td class="px-4 py-3">
								<Skeleton class="h-5 w-16" />
							</td>
							<td class="px-4 py-3">
								<Skeleton class="h-3 w-20" />
							</td>
							<td class="px-4 py-3 text-right">
								<Skeleton class="ml-auto h-6 w-16" />
							</td>
						</tr>
					{/each}
				{:then list}
					{#each list.users as u (u.id)}
						<tr class="transition-colors hover:bg-foreground/2">
							<td class="px-4 py-3">
								<a href="/admin/users/{u.id}" class="block hover:text-primary">
									<span class="block truncate font-medium">{u.name}</span>
									<span class="block truncate text-xs text-muted-foreground">{u.email}</span>
								</a>
							</td>
							<td class="px-4 py-3">
								<div class="flex flex-wrap items-center gap-1.5">
									{#if u.role === "admin"}
										<Badge variant="secondary" class="gap-1">
											<Crown class="size-3" /> admin
										</Badge>
									{:else}
										<Badge variant="outline">user</Badge>
									{/if}
									{#if u.status === "pending"}
										<Badge variant="outline" class="text-amber-600 dark:text-amber-400">
											waitlist
										</Badge>
									{/if}
									{#if u.banned}
										<Badge variant="destructive" class="gap-1">
											<ShieldOff class="size-3" /> banned
										</Badge>
									{/if}
								</div>
							</td>
							<td class="px-4 py-3 text-muted-foreground">
								{new Date(u.createdAt).toLocaleDateString()}
							</td>
							<td class="px-4 py-3 text-right">
								<a
									href="/admin/users/{u.id}"
									class={cn(
										"inline-flex items-center gap-1.5 rounded-md border border-border/40 px-2.5 py-1 text-xs font-medium transition-colors hover:bg-foreground/5",
									)}
								>
									Manage
								</a>
							</td>
						</tr>
					{:else}
						<tr>
							<td colspan="4" class="px-4 py-10 text-center text-sm text-muted-foreground">
								No users match these filters.
							</td>
						</tr>
					{/each}
				{:catch}
					<tr>
						<td colspan="4" class="px-4 py-6">
							<InlineError message="Couldn't load users." />
						</td>
					</tr>
				{/await}
			</tbody>
		</table>
	</div>
</div>

<!-- Mobile / tablet: one card per user (no horizontal scroll). -->
<div class="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:hidden">
	{#await data.list}
		{#each Array(6) as _, i (i)}
			<div class="glass-card rounded-xl p-3">
				<div class="space-y-1.5">
					<Skeleton class="h-3.5 w-32" />
					<Skeleton class="h-3 w-44" />
				</div>
				<div class="mt-3 flex items-center justify-between">
					<Skeleton class="h-5 w-16" />
					<Skeleton class="h-3 w-14" />
				</div>
			</div>
		{/each}
	{:then list}
		{#each list.users as u (u.id)}
			<a href="/admin/users/{u.id}" class="glass-card block rounded-xl p-3 transition-colors hover:bg-foreground/2">
				<div class="flex items-start justify-between gap-2">
					<div class="min-w-0">
						<span class="block truncate font-medium">{u.name}</span>
						<span class="block truncate text-xs text-muted-foreground">{u.email}</span>
					</div>
					<div class="flex shrink-0 flex-wrap justify-end gap-1">
						{#if u.role === "admin"}
							<Badge variant="secondary" class="gap-1"><Crown class="size-3" /> admin</Badge>
						{:else}
							<Badge variant="outline">user</Badge>
						{/if}
						{#if u.status === "pending"}
							<Badge variant="outline" class="text-amber-600 dark:text-amber-400">waitlist</Badge>
						{/if}
						{#if u.banned}
							<Badge variant="destructive" class="gap-1"><ShieldOff class="size-3" /> banned</Badge>
						{/if}
					</div>
				</div>
				<div class="mt-2.5 flex items-center justify-between text-xs text-muted-foreground">
					<span>Joined {new Date(u.createdAt).toLocaleDateString()}</span>
					<span class="font-medium text-foreground/70">Manage →</span>
				</div>
			</a>
		{:else}
			<div class="glass-card col-span-full rounded-xl px-4 py-10 text-center text-sm text-muted-foreground">
				No users match these filters.
			</div>
		{/each}
	{:catch}
		<div class="col-span-full">
			<InlineError message="Couldn't load users." />
		</div>
	{/await}
</div>

<div class="mt-4 flex items-center justify-between text-xs text-muted-foreground">
	<span>Page {Math.floor(data.offset / data.limit) + 1}</span>
	{#await data.list}
		<div class="flex items-center gap-2">
			<Button variant="outline" size="sm" disabled>
				<ChevronLeft class="size-3.5" /> Prev
			</Button>
			<Button variant="outline" size="sm" disabled>
				Next <ChevronRight class="size-3.5" />
			</Button>
		</div>
	{:then list}
		{@const endIdx = Math.min(data.offset + data.limit, list.total)}
		<div class="flex items-center gap-2">
			<Button
				variant="outline"
				size="sm"
				disabled={data.offset === 0}
				onclick={() => changePage(-1)}
			>
				<ChevronLeft class="size-3.5" /> Prev
			</Button>
			<Button
				variant="outline"
				size="sm"
				disabled={endIdx >= list.total}
				onclick={() => changePage(1)}
			>
				Next <ChevronRight class="size-3.5" />
			</Button>
		</div>
	{:catch}
		<!-- value hidden; the section below surfaces the error + retry -->
	{/await}
</div>
