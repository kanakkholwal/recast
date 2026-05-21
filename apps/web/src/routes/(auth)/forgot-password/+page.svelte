<script lang="ts">
	import AuthCard from "$lib/auth/components/AuthCard.svelte";
	import { authClient } from "$lib/auth/client";
	import { ArrowRight, MailCheck } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let email = $state("");
	let loading = $state(false);
	let sent = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		loading = true;
		const { error } = await authClient.requestPasswordReset({
			email,
			redirectTo: "/reset-password",
		});
		loading = false;
		if (error) {
			toast.error(error.message ?? "Couldn't send the reset email.");
			return;
		}
		// We tell the user the link was sent regardless of whether the email
		// exists — standard pattern, prevents account enumeration.
		sent = true;
	}
</script>

<svelte:head>
	<title>Reset your password — Recast</title>
</svelte:head>

<AuthCard
	title="Forgot your password?"
	description="Drop your email and we'll send you a reset link."
>
	{#if sent}
		<div
			class="flex flex-col items-center gap-3 text-center"
			in:fly={{ y: 8, duration: 360, easing: cubicOut }}
		>
			<span class="glass-chip grid size-11 place-items-center rounded-xl text-primary">
				<MailCheck class="size-5" />
			</span>
			<div>
				<h2 class="text-sm font-semibold text-foreground">Check your inbox</h2>
				<p class="mt-1 text-xs text-muted-foreground">
					If <span class="font-medium text-foreground">{email}</span> matches an account,
					you'll get a reset link shortly.
				</p>
			</div>
			<Button
				variant="outline"
				size="sm"
				class="mt-2 w-full"
				onclick={() => {
					sent = false;
					email = "";
				}}
			>
				Use a different email
			</Button>
		</div>
	{:else}
		<form class="flex flex-col gap-3.5" onsubmit={submit}>
			<Label class="flex flex-col items-stretch gap-1.5">
				<span class="text-xs font-semibold text-foreground/85">Email</span>
				<Input
					type="email"
					required
					autocomplete="email"
					bind:value={email}
					placeholder="you@startup.com"
					class="h-10"
				/>
			</Label>
			<Button
				type="submit"
				disabled={loading}
				class="group/cta mt-1 w-full gap-2"
			>
				{loading ? "Sending…" : "Send reset link"}
				<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
			</Button>
		</form>
	{/if}

	{#snippet footer()}
		Remembered it?
		<a href="/login" class="font-semibold text-foreground hover:text-primary">
			Sign in
		</a>
	{/snippet}
</AuthCard>
