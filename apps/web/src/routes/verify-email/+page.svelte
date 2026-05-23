<script lang="ts">
	import { goto, invalidateAll } from "$app/navigation";
	import { authClient } from "$lib/auth/client";
	import Logo from "$lib/logo.svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import {
		ArrowRight,
		LoaderCircle,
		LogOut,
		MailCheck,
		RefreshCw,
	} from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

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
					success: "Sent — check your inbox.",
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
	<title>Verify your email — Recast</title>
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
				One step left
			</span>

			<h1 class="text-balance mt-5 text-3xl font-semibold leading-tight tracking-tight text-foreground sm:text-4xl">
				Verify your email
			</h1>
			<p class="text-pretty mt-3 max-w-sm text-sm leading-relaxed text-muted-foreground">
				We sent a confirmation link to
				<span class="font-mono font-medium text-foreground">{data.email}</span>.
				Click it and you'll land back here — until then your dashboard stays
				read-only.
			</p>
		</div>

		<div class="glass-card mt-8 rounded-2xl p-6 shadow-craft-lg sm:p-7">
			<div class="space-y-2.5">
				<Button onclick={refresh} disabled={checking} class="group/cta w-full gap-2">
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
			<p class="mt-5 text-center text-[11px] text-muted-foreground">
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
		</div>
	</div>
</div>
