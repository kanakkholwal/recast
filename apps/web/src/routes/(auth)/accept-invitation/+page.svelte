<script lang="ts">
import { AlertTriangle, ArrowRight, Check, LoaderCircle, MailCheck, Wand2, X } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { goto, invalidateAll } from "$app/navigation";
import { authClient } from "$lib/auth/client";
import AuthCard from "$lib/auth/components/AuthCard.svelte";
import { isInviteBlocked } from "./invitation.logic";

let { data } = $props();

let accepting = $state(false);
let rejecting = $state(false);
let sendingLink = $state(false);
let linkSent = $state(false);
/** Either action in flight — prevents accept + reject racing each other. */
const busy = $derived(accepting || rejecting);

const blocked = $derived(isInviteBlocked(data.invite, data.viewer));

async function accept() {
	if (busy) return;
	accepting = true;
	const toastId = toast.loading(`Joining ${data.invite.orgName}…`);
	try {
		const { error } = await authClient.organization.acceptInvitation({
			invitationId: data.invite.id,
		});
		if (error) throw new Error(error.message ?? "Couldn't accept the invitation.");
		toast.success(`Welcome to ${data.invite.orgName}.`, { id: toastId });
		// Re-run every loader so the dashboard's team gate sees the new membership rather than bouncing back to onboarding.
		await invalidateAll();
		await goto("/dashboard", { invalidateAll: true });
	} catch (err) {
		toast.error((err as Error)?.message ?? "Couldn't accept the invitation.", {
			id: toastId,
		});
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
				success: "Check your inbox. The link expires in 10 minutes.",
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
	<title>Team invitation - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<AuthCard
	eyebrowIcon={MailCheck}
	eyebrow="Team invitation"
	title={`Join ${data.invite.orgName}`}
	description={`You will join as ${data.invite.role}.`}
>
			{#if data.invite.status !== "pending"}
				<div class="flex flex-col gap-3 text-body-sm text-muted-foreground">
					<AlertTriangle class="size-5 text-tag-tangerine" />
					<span>
						This invitation has already been
						<span class="font-medium text-foreground">{data.invite.status}</span>.
					</span>
				</div>
			{:else if data.invite.expired}
				<div class="flex flex-col gap-3 text-body-sm text-muted-foreground">
					<AlertTriangle class="size-5 text-tag-tangerine" />
					<span>This invitation has expired. Ask the team owner to resend it.</span>
				</div>
			{:else if !data.viewer}
				<!-- Not signed in — magic-link sign-in directly on this page so the
				     invitee doesn't have to bounce through /login. We know the
				     email (it's the invite target) and we pre-created the user
				     row server-side, so the link will go through. -->
				{#if linkSent}
					<div
						class="flex flex-col gap-3 text-body-sm"
						in:fly={{ y: 6, duration: 280, easing: cubicOut }}
					>
						<span class="pill grid size-11 place-items-center rounded-xl text-primary">
							<MailCheck class="size-5" />
						</span>
						<div>
							<p class="font-semibold text-foreground">Check your inbox</p>
							<p class="mt-1 text-caption text-muted-foreground">
								We sent a one-time sign-in link to
								<span class="font-mono font-semibold text-foreground">{data.invite.email}</span>.
								Click it and you'll land back here to accept.
							</p>
						</div>
					</div>
				{:else}
					<div class="space-y-3 text-body-sm">
						<p class="text-muted-foreground">
							Sign in as
							<span class="font-mono font-semibold text-foreground">{data.invite.email}</span>
							to accept this invitation. We'll email you a one-time link, no
							password needed.
						</p>
						<Button onclick={sendSignInLink} disabled={sendingLink} variant="dark" class="group/cta w-full gap-2">
							{#if sendingLink}
								<LoaderCircle class="size-4 animate-spin" />
							{:else}
								<Wand2 class="size-4" />
							{/if}
							{sendingLink ? "Sending…" : "Email me a sign-in link"}
						</Button>
						<p class="text-caption text-muted-foreground">
							Already have a password?
							<a
								href={`/login?next=${encodeURIComponent(`/accept-invitation?id=${data.invite.id}`)}`}
								class="font-semibold text-foreground hover:text-primary"
							>
								Sign in with password
							</a>
						</p>
					</div>
				{/if}
			{:else if !data.viewer.emailMatches}
				<div class="space-y-3 text-body-sm text-muted-foreground">
					<div class="flex items-start gap-2.5 rounded-lg border border-border-low bg-paper p-3.5">
						<AlertTriangle class="mt-0.5 size-4 shrink-0 text-tag-tangerine" />
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
					<Button onclick={accept} disabled={busy || blocked} variant="dark" class="group/cta w-full gap-2">
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
				<p class="mt-4 text-caption text-muted-foreground">
					Signed in as <span class="font-medium text-foreground">{data.viewer.email}</span>
			</p>
		{/if}
	
</AuthCard>
