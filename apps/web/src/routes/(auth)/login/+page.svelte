<script lang="ts">
	import { dev } from "$app/environment";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import AuthCard from "$lib/auth/components/AuthCard.svelte";
	import OrDivider from "$lib/auth/components/OrDivider.svelte";
	import SocialButtons from "$lib/auth/components/SocialButtons.svelte";
	import { authClient } from "$lib/auth/client";
	import { ArrowRight, Eye, EyeOff, MailCheck, Wand2 } from "@lucide/svelte";
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

	const next = $derived(page.url.searchParams.get("next") || "/dashboard");

	async function signInWithLink(e: SubmitEvent) {
		e.preventDefault();
		if (!email.trim()) return;
		loading = true;
		const { error } = await authClient.signIn.magicLink({
			email,
			callbackURL: next,
		});
		loading = false;
		if (error) {
			toast.error(error.message ?? "Couldn't send the sign-in link.");
			return;
		}
		linkSent = true;
	}

	async function signInWithPassword(e: SubmitEvent) {
		e.preventDefault();
		loading = true;
		const { error } = await authClient.signIn.email({
			email,
			password,
			rememberMe,
		});
		loading = false;
		if (error) {
			toast.error(error.message ?? "Sign in failed. Check your credentials.");
			return;
		}
		await goto(next);
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
			<Tabs.List variant="line" class="mb-5 w-full justify-stretch">
				<Tabs.Trigger value="link" class="flex-1 gap-1.5">
					<Wand2 class="size-3.5" />
					Magic link
				</Tabs.Trigger>
				<Tabs.Trigger value="password" class="flex-1">
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
						<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
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
						<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
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
