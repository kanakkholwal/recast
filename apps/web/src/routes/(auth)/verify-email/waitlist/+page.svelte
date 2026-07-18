<script lang="ts">
	import { page } from "$app/state";
	import { SeoMeta } from "$lib/components";
	import WaitlistForm from "$lib/components/WaitlistForm.svelte";
	import Logo from "$lib/logo.svelte";
	import { Lock } from "@recast/icons";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	const source = $derived(page.url.searchParams.get("source") ?? "waitlist");
	// Prefill from `?email=` when the user lands here via the "Join waitlist"
	// CTA on /login, saves them from retyping.
	const initialEmail = untrack(() => page.url.searchParams.get("email")?.trim() ?? "");
</script>

<SeoMeta
	title="Join the Recast waitlist"
	description="Recast accounts are invite-only right now. Drop your email and we'll let you in when sign-ups open."
	eyebrow="Waitlist"
	pageTitle="Join the Recast waitlist"
/>

<div class="w-full max-w-md" in:fly={{ y: 16, duration: 600, easing: cubicOut }}>
	<div class="flex flex-col items-center text-center">
		<a
			href="/"
			class="group/logo flex items-center gap-2.5"
			aria-label="Recast home"
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
			<Lock class="size-3" />
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
		<WaitlistForm {source} {initialEmail} />
	</div>

	<p class="mt-6 text-center text-xs text-muted-foreground">
		Already invited?
		<a href="/login" class="font-semibold text-foreground hover:text-primary">
			Sign in
		</a>
	</p>
</div>