<script lang="ts">
import { AlertCircle, ArrowRight, Eye, EyeOff, LoaderCircle } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Checkbox } from "@recast/ui/checkbox";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import { toast } from "@recast/ui/sonner";
import { untrack } from "svelte";
import { cubicOut } from "svelte/easing";
import { fly, slide } from "svelte/transition";
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { authClient } from "$lib/auth/client";
import AuthCard from "$lib/auth/components/AuthCard.svelte";
import OrDivider from "$lib/auth/components/OrDivider.svelte";
import SocialButtons from "$lib/auth/components/SocialButtons.svelte";
import { lookupEmailStatus } from "$lib/auth/lookup";
import {
	canSignUp,
	passwordsMatch,
	STRENGTH_COLORS,
	STRENGTH_LABELS,
	scorePasswordStrength,
} from "$lib/auth/password.logic";
import { safeNext } from "$lib/auth/redirect";

let { data } = $props();

let name = $state("");
// Seeded once from the `?email=` handoff, then plain editable state.
let email = $state(untrack(() => page.url.searchParams.get("email")?.trim() ?? ""));
let password = $state("");
let confirmPassword = $state("");
let agreed = $state(false);
let showPassword = $state(false);
let loading = $state(false);
/** Set when the lookup says this email already has an account. */
let existing = $state<string | null>(null);

const next = $derived(safeNext(page.url.searchParams.get("next")));
const loginHref = $derived(
	`/login?next=${encodeURIComponent(next)}${email.trim() ? `&email=${encodeURIComponent(email.trim())}` : ""}`,
);

const passwordStrength = $derived(scorePasswordStrength(password));
const matches = $derived(passwordsMatch(password, confirmPassword));
const canSubmit = $derived(canSignUp({ name, email, password, confirmPassword, agreed }));

$effect(() => {
	if (existing && existing !== email.trim()) existing = null;
});

async function signUp(e: SubmitEvent) {
	e.preventDefault();
	if (!canSubmit || loading) return;
	loading = true;
	const trimmedEmail = email.trim();
	try {
		// 'User already exists' is the most common failure, so catch it first and hand them a link to /login; only a definite verdict blocks.
		const status = await lookupEmailStatus(trimmedEmail);
		if (status === "active" || status === "pending") {
			existing = trimmedEmail;
			return;
		}
		await toast.promise(
			(async () => {
				const { error } = await authClient.signUp.email({
					name,
					email: trimmedEmail,
					password,
				});
				if (error) throw new Error(error.message ?? "Couldn't create your account.");
			})(),
			{
				loading: "Creating your account…",
				success: "Account created. Welcome to Recast.",
				error: (err) => (err as Error)?.message ?? "Couldn't create your account.",
			},
		);
		// invalidateAll so the destination's server load reads the fresh session cookie, not its cached signed-out data.
		await goto(next, { invalidateAll: true });
	} finally {
		loading = false;
	}
}
</script>

<svelte:head>
	<title>Start free - Recast</title>
</svelte:head>

<AuthCard
	title="Start sharing free"
	description="Record once. Ship a demo, not a draft."
>
	<SocialButtons providers={data.socialProviders} callbackURL={next} />

	{#if data.socialProviders.length > 0}
		<div class="my-5">
			<OrDivider label="or sign up with email" />
		</div>
	{/if}

	{#if existing}
		<div
			class="mb-4 flex items-start gap-2.5 rounded-lg border border-border-low bg-paper p-3.5 text-caption"
			in:fly={{ y: 6, duration: 280, easing: cubicOut }}
		>
			<AlertCircle class="mt-0.5 size-4 shrink-0 text-tag-tangerine" />
			<div class="min-w-0 flex-1">
				<p class="font-medium text-foreground">
					<span class="font-mono">{existing}</span> already has an account
				</p>
				<p class="mt-0.5 text-muted-foreground">
					Sign in instead — or reset the password if you don't remember it.
				</p>
				<a
					href={loginHref}
					class="mt-2 inline-flex items-center gap-1.5 font-semibold text-primary hover:underline"
				>
					Sign in
					<ArrowRight class="size-3.5" />
				</a>
			</div>
		</div>
	{/if}

	<form class="flex flex-col gap-3.5" onsubmit={signUp}>
		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-caption font-semibold text-foreground">Full name</span>
			<Input
				type="text"
				required
				autocomplete="name"
				bind:value={name}
				placeholder="Jane Founder"
				class="h-10 border-border-low bg-background"
			/>
		</Label>

		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-caption font-semibold text-foreground">Email</span>
			<Input
				type="email"
				required
				autocomplete="email"
				bind:value={email}
				placeholder="you@startup.com"
				class="h-10 border-border-low bg-background"
			/>
		</Label>

		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-caption font-semibold text-foreground">Password</span>
			<div class="relative">
				<Input
					type={showPassword ? "text" : "password"}
					required
					autocomplete="new-password"
					bind:value={password}
					placeholder="At least 8 characters"
					class="h-10 border-border-low bg-background pr-9"
				/>
				<button
					type="button"
					onclick={() => (showPassword = !showPassword)}
					aria-label={showPassword ? "Hide password" : "Show password"}
					class="absolute right-1.5 top-1/2 grid size-7 -translate-y-1/2 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-paper hover:text-foreground"
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
									? STRENGTH_COLORS[passwordStrength]
									: 'bg-foreground/10'}"
							></span>
						{/each}
					</div>
					<span class="w-14 text-right text-caption font-medium text-muted-foreground">
						{STRENGTH_LABELS[passwordStrength]}
					</span>
				</div>
			{/if}
		</Label>

		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-caption font-semibold text-foreground">Confirm password</span>
			<Input
				type={showPassword ? "text" : "password"}
				required
				autocomplete="new-password"
				bind:value={confirmPassword}
				placeholder="Type it again"
				aria-invalid={!matches}
				class="h-10 border-border-low bg-background"
			/>
			{#if !matches}
				<span
					class="flex items-center gap-1 text-caption font-medium text-destructive"
					transition:slide={{ duration: 200, easing: cubicOut }}
				>
					<AlertCircle class="size-3" />
					Passwords don't match
				</span>
			{/if}
		</Label>

		<Label class="flex items-start gap-2">
			<Checkbox bind:checked={agreed} id="terms" class="mt-0.5" />
			<span class="text-caption font-medium text-foreground">
				I agree to Recast's
				<a href="/terms-of-service" class="text-primary hover:underline">Terms</a>
				and
				<a href="/privacy-policy" class="text-primary hover:underline">Privacy Policy</a>.
			</span>
		</Label>

		<Button
			variant="dark"
			type="submit"
			disabled={loading || !canSubmit}
			class="group/cta mt-2 w-full gap-2"
		>
			{loading ? "Setting up your workspace…" : "Start free"}
			{#if loading}
				<LoaderCircle class="size-4 animate-spin" />
			{:else}
				<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
			{/if}
		</Button>
	</form>

	{#snippet footer()}
		Free tier, no card. Already have an account?
		<a href={loginHref} class="font-semibold text-foreground hover:text-primary">
			Sign in
		</a>
	{/snippet}
</AuthCard>
