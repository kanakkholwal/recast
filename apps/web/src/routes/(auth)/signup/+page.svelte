<script lang="ts">
	import { goto } from "$app/navigation";
	import AuthCard from "$lib/auth/components/AuthCard.svelte";
	import OrDivider from "$lib/auth/components/OrDivider.svelte";
	import SocialButtons from "$lib/auth/components/SocialButtons.svelte";
	import { authClient } from "$lib/auth/client";
	import { AlertCircle, ArrowRight, Eye, EyeOff } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import { Checkbox } from "@recast/ui/checkbox";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { cubicOut } from "svelte/easing";
	import { slide } from "svelte/transition";

	let name = $state("");
	let email = $state("");
	let password = $state("");
	let confirmPassword = $state("");
	let agreed = $state(false);
	let showPassword = $state(false);
	let loading = $state(false);

	const passwordStrength = $derived.by(() => {
		const p = password;
		let score = 0;
		if (p.length >= 8) score++;
		if (/[A-Z]/.test(p) && /[a-z]/.test(p)) score++;
		if (/\d/.test(p)) score++;
		if (/[^A-Za-z0-9]/.test(p)) score++;
		return score; // 0..4
	});
	const strengthLabel = ["Weak", "Fair", "Good", "Strong", "Excellent"];
	const strengthColor = [
		"bg-destructive/60",
		"bg-warning/60",
		"bg-warning",
		"bg-success/80",
		"bg-success",
	];

	const passwordsMatch = $derived(
		password === confirmPassword || confirmPassword.length === 0,
	);

	const canSubmit = $derived(
		name.trim().length > 0 &&
			email.trim().length > 0 &&
			password.length >= 8 &&
			password === confirmPassword &&
			agreed,
	);

	async function signUp(e: SubmitEvent) {
		e.preventDefault();
		if (!canSubmit) return;
		loading = true;
		const { error } = await authClient.signUp.email({ name, email, password });
		loading = false;
		if (error) {
			toast.error(error.message ?? "Couldn't create your account.");
			return;
		}
		await goto("/dashboard");
	}
</script>

<svelte:head>
	<title>Sign up — Recast</title>
</svelte:head>

<AuthCard
	title="Create your account"
	description="Start recording demos that look expensive — without an editor."
>
	<SocialButtons />

	<div class="my-5">
		<OrDivider label="or sign up with email" />
	</div>

	<form class="flex flex-col gap-3.5" onsubmit={signUp}>
		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-xs font-semibold text-foreground/85">Full name</span>
			<Input
				type="text"
				required
				autocomplete="name"
				bind:value={name}
				placeholder="Jane Founder"
				class="h-10"
			/>
		</Label>

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

		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-xs font-semibold text-foreground/85">Password</span>
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

			{#if password.length > 0}
				<div
					class="flex items-center gap-2 pt-0.5"
					transition:slide={{ duration: 200, easing: cubicOut }}
				>
					<div class="flex flex-1 gap-1">
						{#each Array(4) as _, i}
							<span
								class="h-1 flex-1 rounded-full transition-colors duration-200
									{i < passwordStrength
									? strengthColor[passwordStrength]
									: 'bg-foreground/10'}"
							></span>
						{/each}
					</div>
					<span class="w-14 text-right text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
						{strengthLabel[passwordStrength]}
					</span>
				</div>
			{/if}
		</Label>

		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-xs font-semibold text-foreground/85">Confirm password</span>
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

		<Label class="flex items-start gap-2">
			<Checkbox bind:checked={agreed} id="terms" class="mt-0.5" />
			<span class="text-xs font-medium text-foreground/85">
				I agree to Recast's
				<a href="/" class="text-primary hover:underline">Terms</a>
				and
				<a href="/" class="text-primary hover:underline">Privacy Policy</a>.
			</span>
		</Label>

		<Button
			type="submit"
			disabled={loading || !canSubmit}
			class="group/cta mt-2 w-full gap-2"
		>
			{loading ? "Creating account…" : "Create account"}
			<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
		</Button>
	</form>

	{#snippet footer()}
		Already have an account?
		<a href="/login" class="font-semibold text-foreground hover:text-primary">
			Sign in
		</a>
	{/snippet}
</AuthCard>
