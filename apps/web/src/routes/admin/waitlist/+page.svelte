<script lang="ts">
	import { enhance } from "$app/forms";
	import { Button } from "@recast/ui/button";
	import { Checkbox } from "@recast/ui/checkbox";
	import { Skeleton } from "@recast/ui/skeleton";
	import { toast } from "@recast/ui/sonner";
	import { LoaderCircle } from "@lucide/svelte";

	let { data } = $props();

	let selected = $state<Set<string>>(new Set());
	let approving = $state(false);

	function toggle(id: string, checked: boolean) {
		const next = new Set(selected);
		if (checked) next.add(id);
		else next.delete(id);
		selected = next;
	}

	function toggleAll(pending: Array<{ id: string }>, checked: boolean) {
		selected = checked ? new Set(pending.map((u) => u.id)) : new Set();
	}
</script>

<header class="mb-6">
	<h1 class="text-2xl font-semibold tracking-tight">Waitlist</h1>
	<p class="mt-1 text-sm text-muted-foreground">
		{#await data.pending}
			Loading…
		{:then pending}
			{pending.length} pending {pending.length === 1 ? "user" : "users"}.
		{/await}
		Approving flips status → active so they can sign in via magic link or password reset.
	</p>
</header>

<form
	method="POST"
	action="?/approve"
	use:enhance={() => {
		approving = true;
		return async ({ result, update }) => {
			try {
				if (result.type === "success") {
					toast.success(`Approved ${result.data?.approved ?? 0} user(s).`);
					selected = new Set();
				} else if (result.type === "failure") {
					toast.error(String(result.data?.error));
				}
				await update();
			} finally {
				approving = false;
			}
		};
	}}
>
	{#await data.pending}
		<div class="mb-3 flex items-center justify-between">
			<Skeleton class="h-4 w-24" />
			<Skeleton class="h-8 w-24" />
		</div>
		<div class="glass-card overflow-hidden rounded-xl">
			<ul class="divide-y divide-border/30">
				{#each Array(5) as _, i (i)}
					<li class="flex items-center gap-3 px-4 py-3">
						<Skeleton class="size-4 shrink-0 rounded" />
						<div class="min-w-0 flex-1 space-y-1.5">
							<Skeleton class="h-3.5 w-32" />
							<Skeleton class="h-3 w-44" />
						</div>
						<Skeleton class="h-3 w-16" />
					</li>
				{/each}
			</ul>
		</div>
	{:then pending}
		{@const allChecked = pending.length > 0 && selected.size === pending.length}
		<div class="mb-3 flex items-center justify-between">
			<label class="flex items-center gap-2 text-xs font-medium">
				<Checkbox
					checked={allChecked}
					onCheckedChange={(v) => toggleAll(pending, Boolean(v))}
				/>
				Select all
			</label>
			<Button type="submit" size="sm" disabled={selected.size === 0 || approving} class="gap-2">
				{#if approving}
					<LoaderCircle class="size-3.5 animate-spin" />
				{/if}
				{approving ? "Approving…" : `Approve ${selected.size || ""}`}
			</Button>
		</div>

		<div class="glass-card overflow-hidden rounded-xl">
			<ul class="divide-y divide-border/30">
				{#each pending as u (u.id)}
					<li class="flex items-center gap-3 px-4 py-3">
						<Checkbox
							checked={selected.has(u.id)}
							onCheckedChange={(v) => toggle(u.id, Boolean(v))}
						/>
						<input type="hidden" name="id" value={selected.has(u.id) ? u.id : ""} />
						<div class="min-w-0 flex-1">
							<a href="/admin/users/{u.id}" class="block truncate text-sm font-medium hover:text-primary">
								{u.name}
							</a>
							<span class="block truncate text-xs text-muted-foreground">{u.email}</span>
						</div>
						<span class="shrink-0 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
							{new Date(u.createdAt).toLocaleDateString()}
						</span>
					</li>
				{:else}
					<li class="px-4 py-10 text-center text-sm text-muted-foreground">
						Nobody is waiting.
					</li>
				{/each}
			</ul>
		</div>
	{/await}
</form>
