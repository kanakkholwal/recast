<script lang="ts">
	import { goto, invalidateAll } from "$app/navigation";
	import { authClient } from "$lib/auth/client";
	import Logo from "$lib/logo.svelte";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import {
		ArrowRight,
		Check,
		LoaderCircle,
		MailCheck,
		Plus,
		Users,
	} from "@lucide/svelte";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let { data } = $props();

	let teamName = $state(
		untrack(() =>
			data.user.name ? `${data.user.name.split(/\s+/)[0]}'s Team` : "My Team",
		),
	);
	let creating = $state(false);
	let acceptingId = $state<string | null>(null);
	/** Either action in flight — prevents create + accept racing each other. */
	const busy = $derived(creating || acceptingId !== null);

	async function createTeam(e: SubmitEvent) {
		e.preventDefault();
		if (!teamName.trim() || busy) return;
		creating = true;
		const target = teamName.trim();
		const slug = `${target.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "")}-${Math.random().toString(36).slice(2, 8)}`;
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.organization.create({
						name: target,
						slug,
					});
					if (error) throw new Error(error.message ?? "Couldn't create the team.");
				})(),
				{
					loading: `Creating ${target}…`,
					success: `Welcome to ${target}.`,
					error: (err) => (err as Error)?.message ?? "Couldn't create the team.",
				},
			);
			await goto("/dashboard");
		} finally {
			// Always release — a thrown rejection (network drop, abort) must
			// not leave the button permanently disabled.
			creating = false;
		}
	}

	async function acceptInvite(id: string) {
		if (busy) return;
		acceptingId = id;
		const target = data.invites.find((i) => i.id === id);
		const orgName = target?.orgName ?? "the team";
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.organization.acceptInvitation({
						invitationId: id,
					});
					if (error) throw new Error(error.message ?? "Couldn't accept the invitation.");
				})(),
				{
					loading: `Joining ${orgName}…`,
					success: `Welcome to ${orgName}.`,
					error: (err) => (err as Error)?.message ?? "Couldn't accept the invitation.",
				},
			);
			await invalidateAll();
			await goto("/dashboard");
		} finally {
			acceptingId = null;
		}
	}
</script>

<svelte:head>
	<title>Set up your team — Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<div class="relative grid min-h-screen place-items-center px-6 py-16 text-foreground">
	<div
		aria-hidden="true"
		class="pointer-events-none absolute inset-0 -z-10"
		style="background: radial-gradient(ellipse 70% 50% at 50% 0%, color-mix(in srgb, var(--color-primary) 9%, transparent), transparent 72%);"
	></div>
	<div
		aria-hidden="true"
		class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 -z-10 opacity-30"
	></div>

	<div
		class="w-full max-w-xl"
		in:fly={{ y: 16, duration: 600, easing: cubicOut }}
	>
		<div class="flex flex-col items-center text-center">
			<a href="/" class="group/logo flex items-center gap-2.5" aria-label="Recast — home">
				<span
					class="grid size-9 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
				>
					<Logo size="22" color="transparent" fill="currentColor" />
				</span>
				<span class="text-lg font-semibold tracking-tight text-foreground">Recast</span>
			</a>

			<h1 class="text-balance mt-7 text-3xl font-semibold leading-tight tracking-tight text-foreground sm:text-4xl">
				Set up your team.
			</h1>
			<p class="text-pretty mt-3 max-w-md text-sm leading-relaxed text-muted-foreground">
				Recast Cloud organizes recordings around teams. Create one to start, or
				accept a pending invite below.
			</p>
		</div>

		<!-- Pending invites — surfaced ABOVE create so users who came via an
		     invitation email don't accidentally spawn a duplicate team. -->
		{#if data.invites.length}
			<section class="glass-card mt-8 rounded-2xl p-5 sm:p-6">
				<div class="mb-4 flex items-center gap-2.5">
					<span class="glass-chip grid size-8 place-items-center rounded-lg text-primary">
						<MailCheck class="size-4" />
					</span>
					<div>
						<h2 class="text-sm font-semibold tracking-tight">Pending invitations</h2>
						<p class="text-[11px] text-muted-foreground">
							Sent to {data.user.email}
						</p>
					</div>
				</div>
				<ul class="space-y-2">
					{#each data.invites as inv (inv.id)}
						<li class="flex items-center justify-between gap-3 rounded-xl border border-border-low/50 bg-foreground/1.5 p-3.5">
							<div class="min-w-0">
								<span class="block truncate text-sm font-medium text-foreground">
									{inv.orgName}
								</span>
								<span class="block truncate text-[11px] text-muted-foreground">
									Joining as {inv.role}
								</span>
							</div>
							<Button
								size="sm"
								disabled={busy}
								onclick={() => acceptInvite(inv.id)}
								class="gap-1.5"
							>
								{acceptingId === inv.id ? "Joining…" : "Accept"}
								{#if acceptingId === inv.id}
									<LoaderCircle class="size-3.5 animate-spin" />
								{:else}
									<Check class="size-3.5" />
								{/if}
							</Button>
						</li>
					{/each}
				</ul>
			</section>
		{/if}

		<section class="glass-card mt-6 rounded-2xl p-5 sm:p-6">
			<div class="mb-4 flex items-center gap-2.5">
				<span class="glass-chip grid size-8 place-items-center rounded-lg text-primary">
					<Users class="size-4" />
				</span>
				<div>
					<h2 class="text-sm font-semibold tracking-tight">Create your team</h2>
					<p class="text-[11px] text-muted-foreground">
						You'll be the owner. Up to {data.caps.free} free teams per account.
					</p>
				</div>
			</div>

			<form class="flex flex-col gap-3" onsubmit={createTeam}>
				<Label class="block">
					<span class="mb-1 block text-xs font-semibold text-foreground/85">
						Team name
					</span>
					<Input
						bind:value={teamName}
						placeholder="Acme demos"
						class="h-10"
						required
					/>
				</Label>
				<Button
					type="submit"
					disabled={busy || !teamName.trim()}
					class="group/cta w-full gap-2"
				>
					{creating ? "Creating…" : "Create team"}
					{#if creating}
						<LoaderCircle class="size-4 animate-spin" />
					{:else}
						<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
					{/if}
				</Button>
			</form>
		</section>

		<p class="mt-7 text-center text-[11px] text-muted-foreground">
			Got the wrong account?
			<button
				type="button"
				class="font-semibold text-foreground transition-colors hover:text-primary"
				onclick={async () => {
					await authClient.signOut();
					await goto("/login");
				}}
			>
				Sign out
			</button>
		</p>
	</div>
</div>
