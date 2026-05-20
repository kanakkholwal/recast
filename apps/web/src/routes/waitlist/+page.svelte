<script lang="ts">
	import { page } from "$app/state";
	import Logo from "$lib/logo.svelte";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { ArrowLeft, ArrowRight, MailCheck, Sparkles } from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	const source = $derived(page.url.searchParams.get("source") ?? "waitlist");

	let email = $state("");
	let loading = $state(false);
	let joined = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (!email.trim()) return;
		loading = true;
		try {
			const res = await fetch("/api/waitlist", {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ email, source }),
			});
			const data = (await res.json().catch(() => ({}))) as {
				ok?: boolean;
				error?: string;
			};
			if (!data.ok) {
				toast.error(data.error ?? "Couldn't join the waitlist.");
				return;
			}
			joined = true;
		} catch {
			toast.error("Network error — try again.");
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Join the Recast waitlist</title>
	<meta
		name="description"
		content="Recast accounts are invite-only right now. Drop your email and we'll let you in when sign-ups open."
	/>
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

	<a
		href="/"
		class="absolute left-6 top-6 inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground transition-colors hover:text-foreground"
	>
		<ArrowLeft class="size-3.5" />
		Back to site
	</a>

	<div
		class="w-full max-w-md"
		in:fly={{ y: 16, duration: 600, easing: cubicOut }}
	>
		<div class="flex flex-col items-center text-center">
			<a
				href="/"
				class="group/logo flex items-center gap-2.5"
				aria-label="Recast — home"
			>
				<span
					class="grid size-9 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
				>
					<Logo size="22" color="transparent" fill="currentColor" />
				</span>
				<span class="text-lg font-semibold tracking-tight text-foreground">
					Recast
				</span>
			</a>

			<span
				class="glass-chip mt-7 inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.16em] text-primary"
			>
				<Sparkles class="size-3" />
				Invite-only
			</span>

			<h1 class="text-balance mt-5 text-3xl font-semibold leading-tight tracking-tight text-foreground sm:text-4xl">
				Recast Cloud is in private beta.
			</h1>
			<p class="text-pretty mt-3 max-w-sm text-sm leading-relaxed text-muted-foreground">
				Sign-ups are paused while we onboard the first wave of founders. Drop
				your email and we'll let you in next.
			</p>
		</div>

		<div class="glass-card mt-8 rounded-2xl p-6 shadow-craft-lg sm:p-7">
			{#if joined}
				<div
					class="flex flex-col items-center gap-3 text-center"
					in:fly={{ y: 8, duration: 360, easing: cubicOut }}
				>
					<span class="glass-chip grid size-11 place-items-center rounded-xl text-primary">
						<MailCheck class="size-5" />
					</span>
					<div>
						<h2 class="text-sm font-semibold text-foreground">You're on the list</h2>
						<p class="mt-1 text-xs text-muted-foreground">
							We'll email <span class="font-medium text-foreground">{email}</span> when
							your spot opens.
						</p>
					</div>
				</div>
			{:else}
				<form class="flex flex-col gap-3.5" onsubmit={submit}>
					<Label class="flex flex-col items-stretch gap-1.5">
						<span class="text-xs font-semibold text-foreground/85">Work email</span>
						<Input
							type="email"
							required
							autocomplete="email"
							bind:value={email}
							placeholder="founder@startup.com"
							class="h-10"
						/>
					</Label>
					<Button
						type="submit"
						disabled={loading}
						class="group/cta mt-1 w-full gap-2"
					>
						{loading ? "Joining…" : "Join the waitlist"}
						<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
					</Button>
				</form>
			{/if}
		</div>

		<p class="mt-6 text-center text-xs text-muted-foreground">
			Already invited?
			<a href="/login" class="font-semibold text-foreground hover:text-primary">
				Sign in
			</a>
		</p>
	</div>
</div>
