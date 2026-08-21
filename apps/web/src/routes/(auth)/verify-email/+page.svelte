<script lang="ts">
import { goto, invalidateAll } from "$app/navigation";
import { authClient } from "$lib/auth/client";
import AuthCard from "$lib/auth/components/AuthCard.svelte";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import { ArrowRight, LoaderCircle, LogOut, MailCheck, RefreshCw } from "@recast/icons";

let { data } = $props();

let sending = $state(false);
let checking = $state(false);
let sentOnce = $state(false);

async function resend() {
	if (sending) return;
	sending = true;
	try {
		await toast.promise(
			(async () => {
				const { error } = await authClient.sendVerificationEmail({
					email: data.email,
					callbackURL: "/dashboard",
				});
				if (error) throw new Error(error.message ?? "Couldn't send the verification email.");
			})(),
			{
				loading: "Sending verification email…",
				success: "Sent. Check your inbox.",
				error: (err) => (err as Error)?.message ?? "Couldn't send the verification email.",
			},
		);
		sentOnce = true;
	} finally {
		sending = false;
	}
}

async function refresh() {
	// User clicked the link in another tab → re-run loaders so the gate
	// sees the new `emailVerified` and lets them through.
	if (checking) return;
	checking = true;
	try {
		await invalidateAll();
	} finally {
		checking = false;
	}
}

async function signOut() {
	await authClient.signOut();
	await goto("/login");
}
</script>

<svelte:head>
	<title>Verify your email - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<AuthCard
	eyebrowIcon={MailCheck}
	eyebrow="One step left"
	title="Verify your email"
	description={`We sent a confirmation link to ${data.email}. Until you click it, your dashboard stays read-only.`}
>
			<div class="space-y-2.5">
				<Button onclick={refresh} disabled={checking} variant="dark" class="group/cta w-full gap-2">
					{#if checking}
						<LoaderCircle class="size-4 animate-spin" />
					{:else}
						<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
					{/if}
					{checking ? "Checking…" : "I clicked the link"}
				</Button>
				<Button
					variant="outline"
					onclick={resend}
					disabled={sending}
					class="w-full gap-2"
				>
					{#if sending}
						<LoaderCircle class="size-4 animate-spin" />
					{:else}
						<RefreshCw class="size-4" />
					{/if}
					{sending ? "Sending…" : sentOnce ? "Send another link" : "Resend verification email"}
				</Button>
			</div>
			<p class="mt-5 text-caption text-muted-foreground">
				Wrong email?
				<button
					type="button"
					onclick={signOut}
					class="inline-flex items-center gap-1 font-semibold text-foreground transition-colors hover:text-primary"
				>
					<LogOut class="size-3" />
					Sign out
				</button>
			</p>
</AuthCard>
