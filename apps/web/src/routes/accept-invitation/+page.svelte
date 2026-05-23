<script lang="ts">
	import { goto } from "$app/navigation";
	import { authClient } from "$lib/auth/client";
	import Logo from "$lib/logo.svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import {
		AlertTriangle,
		ArrowRight,
		Check,
		LoaderCircle,
		MailCheck,
		Wand2,
		X,
	} from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let { data } = $props();

	let accepting = $state(false);
	let rejecting = $state(false);
	let sendingLink = $state(false);
	let linkSent = $state(false);
	/** Either action in flight — prevents accept + reject racing each other. */
	const busy = $derived(accepting || rejecting);

	function sessionMismatch() {
		// Only meaningful once we have a viewer. The unauthed branch lives in
		// its own template block.
		return data.viewer && !data.viewer.emailMatches;
	}

	const blocked = $derived(
		sessionMismatch() ||
			data.invite.expired ||
			data.invite.status !== "pending",
	);

	async function accept() {
		if (busy) return;
		accepting = true;
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.organization.acceptInvitation({
						invitationId: data.invite.id,
					});
					if (error) throw new Error(error.message ?? "Couldn't accept the invitation.");
				})(),
				{
					loading: `Joining ${data.invite.orgName}…`,
					success: `Welcome to ${data.invite.orgName}.`,
					error: (err) => (err as Error)?.message ?? "Couldn't accept the invitation.",
				},
			);
			await goto("/dashboard");
		} finally {
			accepting = false;
		}
	}

	async function reject() {
		if (busy) return;
		rejecting = true;
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.organization.rejectInvitation({
						invitationId: data.invite.id,
					});
					if (error) throw new Error(error.message ?? "Couldn't decline the invitation.");
				})(),
				{
					loading: "Declining…",
					success: "Invitation declined.",
					error: (err) => (err as Error)?.message ?? "Couldn't decline the invitation.",
				},
			);
			await goto("/");
		} finally {
			rejecting = false;
		}
	}

	async function sendSignInLink() {
		if (sendingLink) return;
		sendingLink = true;
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.signIn.magicLink({
						email: data.invite.email,
						// Round-trip the user back here once they click the link.
						callbackURL: `/accept-invitation?id=${data.invite.id}`,
					});
					if (error) throw new Error(error.message ?? "Couldn't send the sign-in link.");
				})(),
				{
					loading: "Sending sign-in link…",
					success: "Check your inbox — the link expires in 10 minutes.",
					error: (err) => (err as Error)?.message ?? "Couldn't send the sign-in link.",
				},
			);
			linkSent = true;
		} finally {
			sendingLink = false;
		}
	}
</script>

<svelte:head>
	<title>Team invitation — Recast</title>
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

	<div class="w-full max-w-md" in:fly={{ y: 16, duration: 520, easing: cubicOut }}>
		<div class="flex flex-col items-center text-center">
			<a href="/" class="group/logo flex items-center gap-2.5" aria-label="Recast — home">
				<span
					class="grid size-9 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
				>
					<Logo size="22" color="transparent" fill="currentColor" />
				</span>
				<span class="text-lg font-semibold tracking-tight text-foreground">Recast</span>
			</a>

			<span class="glass-chip mt-7 inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.16em] text-primary">
				<MailCheck class="size-3" />
				Team invitation
			</span>

			<h1 class="text-balance mt-5 text-3xl font-semibold leading-tight tracking-tight text-foreground sm:text-4xl">
				Join {data.invite.orgName}
			</h1>
			<p class="text-pretty mt-3 max-w-sm text-sm leading-relaxed text-muted-foreground">
				You'll join as <span class="font-medium text-foreground">{data.invite.role}</span>.
			</p>
		</div>

		<div class="glass-card mt-8 rounded-2xl p-6 shadow-craft-lg sm:p-7">
			{#if data.invite.status !== "pending"}
				<div class="flex flex-col items-center gap-3 text-center text-sm text-muted-foreground">
					<AlertTriangle class="size-5 text-amber-500" />
					<span>
						This invitation has already been
						<span class="font-medium text-foreground">{data.invite.status}</span>.
					</span>
				</div>
			{:else if data.invite.expired}
				<div class="flex flex-col items-center gap-3 text-center text-sm text-muted-foreground">
					<AlertTriangle class="size-5 text-amber-500" />
					<span>This invitation has expired. Ask the team owner to resend it.</span>
				</div>
			{:else if !data.viewer}
				<!-- Not signed in — magic-link sign-in directly on this page so the
				     invitee doesn't have to bounce through /login. We know the
				     email (it's the invite target) and we pre-created the user
				     row server-side, so the link will go through. -->
				{#if linkSent}
					<div
						class="flex flex-col items-center gap-3 text-center text-sm"
						in:fly={{ y: 6, duration: 280, easing: cubicOut }}
					>
						<span class="glass-chip grid size-11 place-items-center rounded-xl text-primary">
							<MailCheck class="size-5" />
						</span>
						<div>
							<p class="font-semibold text-foreground">Check your inbox</p>
							<p class="mt-1 text-xs text-muted-foreground">
								We sent a one-time sign-in link to
								<span class="font-mono font-semibold text-foreground">{data.invite.email}</span>.
								Click it and you'll land back here to accept.
							</p>
						</div>
					</div>
				{:else}
					<div class="space-y-3 text-sm">
						<p class="text-muted-foreground">
							Sign in as
							<span class="font-mono font-semibold text-foreground">{data.invite.email}</span>
							to accept this invitation. We'll email you a one-time link — no
							password needed.
						</p>
						<Button onclick={sendSignInLink} disabled={sendingLink} class="group/cta w-full gap-2">
							{#if sendingLink}
								<LoaderCircle class="size-4 animate-spin" />
							{:else}
								<Wand2 class="size-4" />
							{/if}
							{sendingLink ? "Sending…" : "Email me a sign-in link"}
						</Button>
						<p class="text-center text-[11px] text-muted-foreground">
							Already have a password?
							<a
								href={`/login?next=/accept-invitation?id=${data.invite.id}`}
								class="font-semibold text-foreground hover:text-primary"
							>
								Sign in with password
							</a>
						</p>
					</div>
				{/if}
			{:else if !data.viewer.emailMatches}
				<div class="space-y-3 text-sm text-muted-foreground">
					<div class="flex items-start gap-2.5 rounded-xl border border-amber-500/30 bg-amber-500/8 p-3.5">
						<AlertTriangle class="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-400" />
						<span>
							This invitation is for
							<span class="font-mono font-semibold text-foreground">{data.invite.email}</span>,
							but you're signed in as
							<span class="font-mono font-semibold text-foreground">{data.viewer.email}</span>.
						</span>
					</div>
					<Button
						variant="outline"
						class="w-full"
						onclick={async () => {
							await authClient.signOut();
							await goto(`/accept-invitation?id=${data.invite.id}`);
						}}
					>
						Sign in with the right account
					</Button>
				</div>
			{:else}
				<div class="flex flex-col gap-2.5">
					<Button onclick={accept} disabled={busy || blocked} class="group/cta w-full gap-2">
						{accepting ? "Joining…" : "Accept invitation"}
						{#if accepting}
							<LoaderCircle class="size-4 animate-spin" />
						{:else}
							<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
						{/if}
					</Button>
					<Button
						variant="ghost"
						onclick={reject}
						disabled={busy || blocked}
						class="w-full gap-2 text-muted-foreground"
					>
						{#if rejecting}
							<LoaderCircle class="size-4 animate-spin" />
						{:else}
							<X class="size-4" />
						{/if}
						{rejecting ? "Declining…" : "Decline"}
					</Button>
				</div>
				<p class="mt-4 text-center text-[11px] text-muted-foreground">
					Signed in as <span class="font-medium text-foreground">{data.viewer.email}</span>
				</p>
			{/if}
		</div>
	</div>
</div>
