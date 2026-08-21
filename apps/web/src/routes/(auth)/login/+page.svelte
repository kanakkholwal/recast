<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { authClient } from "$lib/auth/client";
import { lookupEmailStatus } from "$lib/auth/lookup";
import { safeNext } from "$lib/auth/redirect";
import AuthCard from "$lib/auth/components/AuthCard.svelte";
import OrDivider from "$lib/auth/components/OrDivider.svelte";
import SocialButtons from "$lib/auth/components/SocialButtons.svelte";
import {
	AlertCircle,
	ArrowRight,
	Eye,
	EyeOff,
	LoaderCircle,
	MailCheck,
	Wand2,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Checkbox } from "@recast/ui/checkbox";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import { toast } from "@recast/ui/sonner";
import * as Tabs from "@recast/ui/tabs";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";

let { data } = $props();

let method = $state<"link" | "password">("link");

let email = $state("");
let password = $state("");
let rememberMe = $state(false);
let showPassword = $state(false);
let loading = $state(false);
let linkSent = $state(false);
/**
 * Inline status banner shown when the lookup reveals this email can't take
 * the path the user picked. Kept inline (rather than a toast) so its CTA
 * stays on screen.
 *   - `unknown`    → no account on file; offer sign-up
 *   - `nopassword` → waitlist-era row that never set one; offer magic link
 */
let preflight = $state<{
	reason: "unknown" | "nopassword";
	email: string;
} | null>(null);

const next = $derived(safeNext(page.url.searchParams.get("next")));
const signupHref = $derived(
	`/signup?next=${encodeURIComponent(next)}${email.trim() ? `&email=${encodeURIComponent(email.trim())}` : ""}`,
);

// Clear the inline banner the moment the user edits their email, so a stale
// banner doesn't linger after they fix a typo.
$effect(() => {
	if (preflight && preflight.email !== email.trim()) preflight = null;
});

/**
 * Returns `true` if we should proceed to the auth call, `false` if the
 * inline banner has been shown and the call should be skipped. `invalid`
 * falls through so the auth call surfaces the real validation error.
 *
 * `pending` is a waitlist-era row: it can sign in now, but it never set a
 * password, so only the password tab has to head it off.
 */
async function preflightEmail(emailInput: string, via: "link" | "password"): Promise<boolean> {
	const status = await lookupEmailStatus(emailInput);
	if (status === "unknown") {
		preflight = { reason: "unknown", email: emailInput };
		return false;
	}
	if (status === "pending" && via === "password") {
		preflight = { reason: "nopassword", email: emailInput };
		return false;
	}
	preflight = null;
	return true;
}

async function signInWithLink(e: SubmitEvent) {
	e.preventDefault();
	const trimmedEmail = email.trim();
	if (!trimmedEmail || loading) return;
	loading = true;
	try {
		const ok = await preflightEmail(trimmedEmail, "link");
		if (!ok) return;
		await toast.promise(
			(async () => {
				const { error } = await authClient.signIn.magicLink({
					email: trimmedEmail,
					callbackURL: next,
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
		loading = false;
	}
}

async function signInWithPassword(e: SubmitEvent) {
	e.preventDefault();
	if (loading) return;
	loading = true;
	const trimmedEmail = email.trim();
	const ok = await preflightEmail(trimmedEmail, "password");
	if (!ok) {
		loading = false;
		return;
	}
	const toastId = toast.loading("Signing you in…");
	try {
		const { error } = await authClient.signIn.email({
			email: trimmedEmail,
			password,
			rememberMe,
		});
		if (error) throw new Error(error.message ?? "Sign in failed. Check your credentials.");
		toast.success("Welcome back.", { id: toastId });
		// Force a fresh load chain so the destination's server load sees
		// the new session cookie immediately, not whatever the client had
		// cached pre-login.
		await goto(next, { invalidateAll: true });
	} catch (err) {
		toast.error((err as Error)?.message ?? "Sign in failed. Check your credentials.", {
			id: toastId,
		});
	} finally {
		loading = false;
	}
}
</script>

<svelte:head>
	<title>Sign in - Recast</title>
</svelte:head>

<AuthCard title="Welcome back" description="Sign in to your Recast account.">
	<SocialButtons providers={data.socialProviders} callbackURL={next} />

	{#if data.socialProviders.length > 0}
		<div class="my-5">
			<OrDivider label="or continue with email" />
		</div>
	{/if}

	{#if preflight}
		<div
			class="mb-4 flex items-start gap-2.5 rounded-xl border border-amber-500/30 bg-amber-500/8 p-3.5 text-xs"
			in:fly={{ y: 6, duration: 280, easing: cubicOut }}
		>
			<AlertCircle class="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-400" />
			<div class="min-w-0 flex-1">
				{#if preflight.reason === "unknown"}
					<p class="font-medium text-foreground">
						No account for <span class="font-mono">{preflight.email}</span>
					</p>
					<p class="mt-0.5 text-muted-foreground">
						Recast Cloud is free to start. No card, no trial clock.
					</p>
					<a
						href={signupHref}
						class="mt-2 inline-flex items-center gap-1.5 font-semibold text-primary hover:underline"
					>
						Start free with this email
						<ArrowRight class="size-3.5" />
					</a>
				{:else}
					<p class="font-medium text-foreground">
						This account has no password yet
					</p>
					<p class="mt-0.5 text-muted-foreground">
						<span class="font-mono">{preflight.email}</span> was created from the
						waitlist. Sign in with a one-time link, then set a password from
						settings.
					</p>
					<button
						type="button"
						onclick={() => {
							method = "link";
							preflight = null;
						}}
						class="mt-2 inline-flex items-center gap-1.5 font-semibold text-primary hover:underline"
					>
						Email me a sign-in link
						<ArrowRight class="size-3.5" />
					</button>
				{/if}
			</div>
		</div>
	{/if}

	{#if linkSent}
		<div
			class="flex flex-col items-center gap-3 text-center"
			in:fly={{ y: 8, duration: 360, easing: cubicOut }}
		>
			<span class="pill grid size-11 place-items-center rounded-xl text-primary">
				<MailCheck class="size-5" />
			</span>
			<div>
				<h2 class="text-sm font-semibold text-foreground">Check your inbox</h2>
				<p class="mt-1 text-xs text-muted-foreground">
					We've sent a sign-in link to
					<span class="font-medium text-foreground">{email}</span>.
					It expires in 10 minutes.
				</p>
			</div>
			<Button
				variant="outline"
				size="sm"
				class="mt-2 w-full"
				onclick={() => {
					linkSent = false;
					email = "";
				}}
			>
				Use a different email
			</Button>
		</div>
	{:else}
		<Tabs.Root bind:value={method} class="w-full">
			<Tabs.List variant="soft" class="mb-5 grid w-full grid-cols-2 gap-1 p-1">
				<Tabs.Trigger value="link" class="gap-1.5">
					<Wand2 class="size-3.5" />
					Magic link
				</Tabs.Trigger>
				<Tabs.Trigger value="password">
					Password
				</Tabs.Trigger>
			</Tabs.List>

			<Tabs.Content value="link">
				<form class="flex flex-col gap-3.5" onsubmit={signInWithLink}>
					<Label class="flex flex-col items-stretch gap-1.5">
						<span class="text-xs font-semibold text-foreground">Email</span>
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
						{loading ? "Sending…" : "Send sign-in link"}
						{#if loading}
							<LoaderCircle class="size-4 animate-spin" />
						{:else}
							<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
						{/if}
					</Button>
					<p class="text-center text-caption text-muted-foreground">
						No password needed. We'll email you a one-time link.
					</p>
				</form>
			</Tabs.Content>

			<Tabs.Content value="password">
				<form class="flex flex-col gap-3.5" onsubmit={signInWithPassword}>
					<Label class="flex flex-col items-stretch gap-1.5">
						<span class="text-xs font-semibold text-foreground">Email</span>
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
						<span class="flex items-center justify-between text-xs font-semibold text-foreground">
							<span>Password</span>
							<a
								href="/forgot-password"
								class="font-medium text-primary transition-colors hover:text-primary/80"
							>
								Forgot password?
							</a>
						</span>
						<div class="relative">
							<Input
								type={showPassword ? "text" : "password"}
								required
								autocomplete="current-password"
								bind:value={password}
								placeholder="••••••••"
								class="h-10 pr-9"
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
					</Label>

					<Label class="flex items-center gap-2">
						<Checkbox bind:checked={rememberMe} id="remember" />
						<span class="text-xs font-medium text-foreground">
							Remember me on this device
						</span>
					</Label>

					<Button
						type="submit"
						disabled={loading}
						class="group/cta mt-1 w-full gap-2"
					>
						{#if loading}
							<LoaderCircle class="size-4 animate-spin" />
						{/if}
						{loading ? "Signing in…" : "Sign in"}
						{#if !loading}
							<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
						{/if}
					</Button>
				</form>
			</Tabs.Content>
		</Tabs.Root>
	{/if}

	{#snippet footer()}
		New to Recast?
		<a href={signupHref} class="font-semibold text-foreground hover:text-primary">
			Start free
		</a>
	{/snippet}
</AuthCard>
