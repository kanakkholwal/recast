<script lang="ts">
	import { page } from "$app/state";
	import { SeoMeta } from "$lib/components";
	import WaitlistForm from "$lib/components/WaitlistForm.svelte";
	import { Lock } from "@lucide/svelte";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	const source = $derived(page.url.searchParams.get("source") ?? "waitlist");
	// Prefill from `?email=` when arriving via a "Join waitlist" CTA (e.g. /login).
	const initialEmail = untrack(() => page.url.searchParams.get("email")?.trim() ?? "");
</script>

<SeoMeta
	title="Join the Recast Cloud waitlist"
	description="Recast Cloud is invite-only right now. Drop your email and we'll let you in before the public launch."
	eyebrow="Waitlist"
	pageTitle="Join the waitlist"
/>

<section class="relative grid min-h-[70vh] place-items-center px-6 py-16 text-foreground">
	<div
		aria-hidden="true"
		class="pointer-events-none absolute inset-0 -z-10"
		style="background: radial-gradient(ellipse 70% 50% at 50% 0%, color-mix(in srgb, var(--color-primary) 9%, transparent), transparent 72%);"
	></div>

	<div class="w-full max-w-md" in:fly={{ y: 16, duration: 600, easing: cubicOut }}>
		<div class="flex flex-col items-center text-center">
			<span class="glass-chip inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.16em] text-primary">
				<Lock class="size-3" />
				Invite-only
			</span>
			<h1 class="text-balance mt-5 text-3xl font-semibold leading-tight tracking-tight text-foreground sm:text-4xl">
				Recast Cloud is in private beta.
			</h1>
			<p class="text-pretty mt-3 max-w-sm text-sm leading-relaxed text-muted-foreground">
				Sign-ups are paused while we onboard the first wave of founders. Drop your email and we'll
				let you in next.
			</p>
		</div>

		<div class="glass-card mt-8 rounded-2xl p-6 shadow-craft-lg sm:p-7">
			<WaitlistForm {source} {initialEmail} />
		</div>

		<p class="mt-6 text-center text-xs text-muted-foreground">
			Already invited?
			<a href="/login" class="font-semibold text-foreground hover:text-primary">Sign in</a>
		</p>
	</div>
</section>
