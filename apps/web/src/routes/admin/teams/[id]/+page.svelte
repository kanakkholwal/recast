<script lang="ts">
	import { enhance } from "$app/forms";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import * as Select from "@recast/ui/select";
	import { toast } from "@recast/ui/sonner";
	import { ArrowLeft, Crown, LoaderCircle, ShieldCheck } from "@lucide/svelte";
	import { untrack } from "svelte";

	let { data } = $props();

	let name = $state(untrack(() => data.team.name));
	let plan = $state(untrack(() => data.team.plan));

	let savingPlan = $state(false);
	let savingName = $state(false);

	const memberCap = $derived(data.caps.members[data.team.plan] ?? 3);
</script>

<a
	href="/admin/teams"
	class="inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground transition-colors hover:text-foreground"
>
	<ArrowLeft class="size-3" />
	All teams
</a>

<header class="mt-3 mb-8">
	<h1 class="truncate text-2xl font-semibold tracking-tight">{data.team.name}</h1>
	<p class="mt-1 font-mono text-xs text-muted-foreground">{data.team.slug}</p>
	<div class="mt-3 flex items-center gap-2">
		<Badge variant={data.team.plan === "free" ? "outline" : "secondary"}>{data.team.plan}</Badge>
		<span class="text-xs text-muted-foreground">
			{data.members.length} / {Number.isFinite(memberCap) ? memberCap : "∞"} seats
		</span>
	</div>
</header>

<div class="grid gap-6 lg:grid-cols-2">
	<section class="glass-card rounded-xl p-5">
		<h2 class="mb-3 text-sm font-semibold tracking-tight">Plan</h2>
		<p class="mb-4 text-xs text-muted-foreground">
			Member cap updates apply immediately. Existing members over the cap aren't
			kicked — but no new invites or accepts will land until you raise the plan
			or remove members.
		</p>
		<form
			method="POST"
			action="?/updatePlan"
			use:enhance={() => {
				savingPlan = true;
				return async ({ result, update }) => {
					if (result.type === "success") toast.success("Plan updated.");
					else if (result.type === "failure") toast.error(String(result.data?.error));
					await update();
					savingPlan = false;
				};
			}}
			class="space-y-3"
		>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Plan</span>
				<Select.Root type="single" bind:value={plan} name="plan">
					<Select.Trigger class="h-9 w-full">{plan}</Select.Trigger>
					<Select.Content>
						<Select.Item value="free">free — 3 seats</Select.Item>
						<Select.Item value="pro">pro — 50 seats</Select.Item>
						<Select.Item value="enterprise">enterprise — no cap</Select.Item>
					</Select.Content>
				</Select.Root>
			</Label>
			<Button type="submit" size="sm" disabled={savingPlan} class="gap-2">
				{#if savingPlan}
					<LoaderCircle class="size-3.5 animate-spin" />
				{/if}
				{savingPlan ? "Saving…" : "Save plan"}
			</Button>
		</form>
	</section>

	<section class="glass-card rounded-xl p-5">
		<h2 class="mb-3 text-sm font-semibold tracking-tight">Rename</h2>
		<form
			method="POST"
			action="?/rename"
			use:enhance={() => {
				savingName = true;
				return async ({ result, update }) => {
					if (result.type === "success") toast.success("Renamed.");
					else if (result.type === "failure") toast.error(String(result.data?.error));
					await update();
					savingName = false;
				};
			}}
			class="space-y-3"
		>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Name</span>
				<Input bind:value={name} name="name" class="h-9" />
			</Label>
			<Button type="submit" size="sm" disabled={savingName} class="gap-2">
				{#if savingName}
					<LoaderCircle class="size-3.5 animate-spin" />
				{/if}
				{savingName ? "Saving…" : "Save name"}
			</Button>
		</form>
	</section>
</div>

<section class="glass-card mt-6 rounded-xl p-5">
	<h2 class="mb-3 text-sm font-semibold tracking-tight">Members</h2>
	<ul class="divide-y divide-border/30">
		{#each data.members as m (m.id)}
			<li class="flex items-center justify-between gap-3 py-2.5">
				<a href="/admin/users/{m.userId}" class="min-w-0 hover:text-primary">
					<span class="block truncate text-sm font-medium">{m.name}</span>
					<span class="block truncate text-xs text-muted-foreground">{m.email}</span>
				</a>
				{#if m.role === "owner"}
					<Badge variant="secondary" class="gap-1"><Crown class="size-3" /> owner</Badge>
				{:else if m.role === "admin"}
					<Badge variant="outline" class="gap-1"><ShieldCheck class="size-3" /> admin</Badge>
				{:else}
					<Badge variant="outline">member</Badge>
				{/if}
			</li>
		{/each}
	</ul>
</section>
