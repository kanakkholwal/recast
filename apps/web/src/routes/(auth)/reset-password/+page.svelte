<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import AuthCard from "$lib/auth/components/AuthCard.svelte";
	import { authClient } from "$lib/auth/client";
	import { AlertCircle, ArrowRight, Eye, EyeOff } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { cubicOut } from "svelte/easing";
	import { slide } from "svelte/transition";

	// Real Better Auth reset flow passes a one-time token in the URL.
	const token = $derived(page.url.searchParams.get("token") ?? "");

	let password = $state("");
	let confirmPassword = $state("");
	let showPassword = $state(false);
	let loading = $state(false);

	const passwordsMatch = $derived(
		password === confirmPassword || confirmPassword.length === 0,
	);
	const canSubmit = $derived(
		password.length >= 8 && password === confirmPassword,
	);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (!canSubmit) return;
		loading = true;
		const { error } = await authClient.resetPassword({
			newPassword: password,
			token,
		});
		loading = false;
		if (error) {
			toast.error(error.message ?? "Couldn't reset your password.");
			return;
		}
		toast.success("Password updated — sign in with your new password.");
		await goto("/login");
	}
</script>

<svelte:head>
	<title>Set a new password — Recast</title>
</svelte:head>

<AuthCard
	title="Set a new password"
	description="Pick something you'll actually remember this time."
>
	<form class="flex flex-col gap-3.5" onsubmit={submit}>
		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-xs font-semibold text-foreground/85">New password</span>
			<div class="relative">
				<Input
					type={showPassword ? "text" : "password"}
					required
					autocomplete="new-password"
					bind:value={password}
					placeholder="At least 8 characters"
					class="h-10 pr-9"
				/>
				<button
					type="button"
					onclick={() => (showPassword = !showPassword)}
					aria-label={showPassword ? "Hide password" : "Show password"}
					class="absolute right-1.5 top-1/2 grid size-7 -translate-y-1/2 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-foreground/8 hover:text-foreground"
				>
					{#if showPassword}
						<EyeOff class="size-3.5" />
					{:else}
						<Eye class="size-3.5" />
					{/if}
				</button>
			</div>
		</Label>

		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-xs font-semibold text-foreground/85">Confirm new password</span>
			<Input
				type={showPassword ? "text" : "password"}
				required
				autocomplete="new-password"
				bind:value={confirmPassword}
				placeholder="Type it again"
				aria-invalid={!passwordsMatch}
				class="h-10"
			/>
			{#if !passwordsMatch}
				<span
					class="flex items-center gap-1 text-[11px] font-medium text-destructive"
					transition:slide={{ duration: 200, easing: cubicOut }}
				>
					<AlertCircle class="size-3" />
					Passwords don't match
				</span>
			{/if}
		</Label>

		<Button
			type="submit"
			disabled={loading || !canSubmit}
			class="group/cta mt-2 w-full gap-2"
		>
			{loading ? "Updating…" : "Update password"}
			<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
		</Button>
	</form>

	{#snippet footer()}
		Back to
		<a href="/login" class="font-semibold text-foreground hover:text-primary">
			Sign in
		</a>
	{/snippet}
</AuthCard>
