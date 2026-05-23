<script lang="ts">
	import { dev } from "$app/environment";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import AuthCard from "$lib/auth/components/AuthCard.svelte";
	import OrDivider from "$lib/auth/components/OrDivider.svelte";
	import SocialButtons from "$lib/auth/components/SocialButtons.svelte";
	import { authClient } from "$lib/auth/client";
	import {
		AlertCircle,
		ArrowRight,
		Eye,
		EyeOff,
		LoaderCircle,
		MailCheck,
		Wand2,
	} from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import { Checkbox } from "@recast/ui/checkbox";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import * as Tabs from "@recast/ui/tabs";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let method = $state<"link" | "password">("link");

	let email = $state("");
	let password = $state("");
	let rememberMe = $state(false);
	let showPassword = $state(false);
	let loading = $state(false);
	let linkSent = $state(false);
	/**
	 * Inline status banner shown when the lookup reveals the email isn't
	 * eligible to sign in yet. Kept inline (rather than a toast) so the CTA
	 * to the waitlist stays visible.
	 *   - `unknown` → no account on file; offer waitlist
	 *   - `pending` → on the waitlist; tell them to wait
	 */
	let preflight = $state<{ status: "unknown" | "pending"; email: string } | null>(
		null,
	);

	const next = $derived(page.url.searchParams.get("next") || "/dashboard");

	// Clear the inline waitlist banner the moment the user edits their email,
	// so the stale banner doesn't linger after they fix a typo.
	$effect(() => {
		if (preflight && preflight.email !== email.trim()) preflight = null;
	});

	/**
	 * Hits /api/auth/lookup to decide whether to actually call Better Auth.
	 * Returns `true` if we should proceed, `false` if the inline banner has
	 * been shown and the auth call should be skipped.
	 */
	async function preflightEmail(emailInput: string): Promise<boolean> {
		try {
			const res = await fetch("/api/auth/lookup", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ email: emailInput }),
			});
			const data = (await res.json()) as {
				status: "active" | "pending" | "unknown" | "invalid";
			};
			if (data.status === "unknown" || data.status === "pending") {
				preflight = { status: data.status, email: emailInput };
				return false;
			}
			// `invalid` falls through to the auth call, which surfaces the
			// real validation error (better than swallowing it here).
			preflight = null;
			return true;
		} catch {
			// Network blip on the lookup shouldn't block sign-in attempts.
			preflight = null;
			return true;
		}
	}

	async function signInWithLink(e: SubmitEvent) {
		e.preventDefault();
		if (!email.trim() || loading) return;
		loading = true;
		try {
			const ok = await preflightEmail(email.trim());
			if (!ok) return;
			await toast.promise(
				(async () => {
					const { error } = await authClient.signIn.magicLink({
						email,
						callbackURL: next,
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
			loading = false;
		}
	}

	async function signInWithPassword(e: SubmitEvent) {
		e.preventDefault();
		if (loading) return;
		loading = true;
		try {
			const ok = await preflightEmail(email.trim());
			if (!ok) return;
			await toast.promise(
				(async () => {
					const { error } = await authClient.signIn.email({
						email,
						password,
						rememberMe,
					});
					if (error) throw new Error(error.message ?? "Sign in failed. Check your credentials.");
				})(),
				{
					loading: "Signing you in…",
					success: "Welcome back.",
					error: (err) => (err as Error)?.message ?? "Sign in failed. Check your credentials.",
				},
			);
			await goto(next);
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Sign in — Recast</title>
</svelte:head>

<AuthCard title="Welcome back" description="Sign in to your Recast account.">
	<SocialButtons />

	{#if dev}
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
				{#if preflight.status === "unknown"}
					<p class="font-medium text-foreground">
						No account for <span class="font-mono">{preflight.email}</span>
					</p>
					<p class="mt-0.5 text-muted-foreground">
						Recast Cloud is invite-only right now. Join the waitlist and
						we'll email you the moment your spot is ready.
					</p>
					<a
						href={`/waitlist?email=${encodeURIComponent(preflight.email)}&source=login`}
						class="mt-2 inline-flex items-center gap-1.5 font-semibold text-primary hover:underline"
					>
						Join the waitlist
						<ArrowRight class="size-3.5" />
					</a>
				{:else}
					<p class="font-medium text-foreground">
						You're on the waitlist
					</p>
					<p class="mt-0.5 text-muted-foreground">
						<span class="font-mono">{preflight.email}</span> is queued. We'll
						email you a sign-in link the moment access opens — no need to
						retry here.
					</p>
				{/if}
			</div>
		</div>
	{/if}

	{#if linkSent}
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
						{loading ? "Sending…" : "Send sign-in link"}
						{#if loading}
							<LoaderCircle class="size-4 animate-spin" />
						{:else}
							<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
						{/if}
					</Button>
					<p class="text-center text-[11px] text-muted-foreground">
						No password needed — we'll email you a one-time link.
					</p>
				</form>
			</Tabs.Content>

			<Tabs.Content value="password">
				<form class="flex flex-col gap-3.5" onsubmit={signInWithPassword}>
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
						<span class="flex items-center justify-between text-xs font-semibold text-foreground/85">
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

					<Label class="flex items-center gap-2">
						<Checkbox bind:checked={rememberMe} id="remember" />
						<span class="text-xs font-medium text-foreground/85">
							Remember me on this device
						</span>
					</Label>

					<Button
						type="submit"
						disabled={loading}
						class="group/cta mt-1 w-full gap-2"
					>
						{loading ? "Signing in…" : "Sign in"}
						{#if loading}
							<LoaderCircle class="size-4 animate-spin" />
						{:else}
							<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
						{/if}
					</Button>
				</form>
			</Tabs.Content>
		</Tabs.Root>
	{/if}

	{#snippet footer()}
		{#if dev}
			Don't have an account?
			<a href="/signup" class="font-semibold text-foreground hover:text-primary">
				Sign up
			</a>
		{:else}
			Need an account?
			<a href="/waitlist" class="font-semibold text-foreground hover:text-primary">
				Join the waitlist
			</a>
		{/if}
	{/snippet}
</AuthCard>
