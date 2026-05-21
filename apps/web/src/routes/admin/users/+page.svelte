<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import {
	  ChevronLeft,
	  ChevronRight,
	  Crown,
	  Search,
	  ShieldOff,
	} from "@lucide/svelte";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import * as Select from "@recast/ui/select";
	import { cn } from "@recast/ui/utils";
	import { untrack } from "svelte";

	let { data } = $props();

	// Seed editable form state once from the URL-driven `data.filters` — we
	// don't want a later page navigation to clobber what the user just typed.
	let q = $state(untrack(() => data.filters.q));
	let searchField = $state<"email" | "name">(untrack(() => data.filters.field));
	let roleFilter = $state<string>(untrack(() => data.filters.role ?? "all"));
	let statusFilter = $state<string>(untrack(() => data.filters.status ?? "all"));

	function applyFilters(reset = true) {
		const sp = new URLSearchParams();
		if (q.trim()) sp.set("q", q.trim());
		sp.set("field", searchField);
		if (roleFilter !== "all") sp.set("role", roleFilter);
		if (statusFilter !== "all") sp.set("status", statusFilter);
		sp.set("sort", data.filters.sort);
		sp.set("dir", data.filters.dir);
		sp.set("limit", String(data.limit));
		if (!reset) sp.set("offset", String(data.offset));
		goto(`/admin/users?${sp.toString()}`, { keepFocus: true });
	}

	function changePage(delta: number) {
		const sp = new URLSearchParams(page.url.searchParams);
		const newOffset = Math.max(0, data.offset + delta * data.limit);
		sp.set("offset", String(newOffset));
		goto(`/admin/users?${sp.toString()}`);
	}

	function toggleSort(field: string) {
		const sp = new URLSearchParams(page.url.searchParams);
		const currentDir = data.filters.sort === field ? data.filters.dir : "desc";
		const nextDir = currentDir === "desc" ? "asc" : "desc";
		sp.set("sort", field);
		sp.set("dir", nextDir);
		sp.delete("offset");
		goto(`/admin/users?${sp.toString()}`);
	}

	const startIdx = $derived(data.total === 0 ? 0 : data.offset + 1);
	const endIdx = $derived(Math.min(data.offset + data.limit, data.total));

	function sortIndicator(field: string): string {
		if (data.filters.sort !== field) return "";
		return data.filters.dir === "asc" ? "↑" : "↓";
	}
</script>

<header class="mb-6 flex flex-wrap items-end justify-between gap-3">
	<div>
		<h1 class="text-2xl font-semibold tracking-tight">Users</h1>
		<p class="mt-1 text-sm text-muted-foreground">
			{data.total.toLocaleString()} total · showing {startIdx}–{endIdx}
		</p>
	</div>
</header>

<form
	class="mb-4 flex flex-wrap items-center gap-2 bg-card/40 p-2 rounded-lg"
	onsubmit={(e) => {
		e.preventDefault();
		applyFilters();
	}}
>
	<div class="relative flex-1 min-w-55">
		<Search class="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
		<Input
			type="search"
			placeholder="Search by {searchField}…"
			bind:value={q}
			class="h-9 pl-9"
		/>
	</div>

	<Select.Root type="single" bind:value={searchField}>
		<Select.Trigger class="h-9 w-32">
			{searchField}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="email">email</Select.Item>
			<Select.Item value="name">name</Select.Item>
		</Select.Content>
	</Select.Root>

	<Select.Root type="single" bind:value={roleFilter}>
		<Select.Trigger class="h-9 w-32">
			role: {roleFilter}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="all">all</Select.Item>
			<Select.Item value="user">user</Select.Item>
			<Select.Item value="admin">admin</Select.Item>
		</Select.Content>
	</Select.Root>

	<Select.Root type="single" bind:value={statusFilter}>
		<Select.Trigger class="h-9 w-36">
			status: {statusFilter}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="all">all</Select.Item>
			<Select.Item value="active">active</Select.Item>
			<Select.Item value="pending">pending</Select.Item>
		</Select.Content>
	</Select.Root>

	<Button type="submit" size="sm">Apply</Button>
	<Button
		type="button"
		size="sm"
		variant="ghost"
		onclick={() => {
			q = "";
			searchField = "email";
			roleFilter = "all";
			statusFilter = "all";
			applyFilters();
		}}
	>
		Reset
	</Button>
</form>

<div class="glass-card overflow-hidden rounded-xl">
	<div class="overflow-x-auto">
		<table class="w-full min-w-160 text-left text-sm">
			<thead class="border-b border-border/40 bg-foreground/2 text-[11px] uppercase tracking-[0.12em] text-muted-foreground">
				<tr>
					<th class="px-4 py-2.5">
						<button class="font-semibold hover:text-foreground" onclick={() => toggleSort("name")}>
							User {sortIndicator("name")}
						</button>
					</th>
					<th class="px-4 py-2.5">Role / Status</th>
					<th class="px-4 py-2.5">
						<button class="font-semibold hover:text-foreground" onclick={() => toggleSort("createdAt")}>
							Joined {sortIndicator("createdAt")}
						</button>
					</th>
					<th class="px-4 py-2.5 text-right">Actions</th>
				</tr>
			</thead>
			<tbody class="divide-y divide-border/30">
				{#each data.users as u (u.id)}
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
			</tbody>
		</table>
	</div>
</div>

<div class="mt-4 flex items-center justify-between text-xs text-muted-foreground">
	<span>Page {Math.floor(data.offset / data.limit) + 1}</span>
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
			disabled={endIdx >= data.total}
			onclick={() => changePage(1)}
		>
			Next <ChevronRight class="size-3.5" />
		</Button>
	</div>
</div>
