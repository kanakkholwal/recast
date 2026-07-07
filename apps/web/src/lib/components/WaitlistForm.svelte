<script lang="ts">
	import { joinWaitlist } from "$lib/waitlist";
	import { ArrowRight, LoaderCircle, MailCheck } from "@lucide/svelte";
	import { Button } from "@recast/ui/button";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { untrack } from "svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	// Shared waitlist email capture (email input → success). `source` tags the
	// funnel; `initialEmail` prefills from a `?email=` handoff (e.g. from /login).
	let { source = "waitlist", initialEmail = "" }: { source?: string; initialEmail?: string } =
		$props();

	// Seed once, then it's independent editable state (untrack = intentional).
	let email = $state(untrack(() => initialEmail));
	let loading = $state(false);
	let joined = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (!email.trim() || loading) return;
		loading = true;
		try {
			await toast.promise(joinWaitlist(email, source), {
				loading: "Adding you to the waitlist…",
				success: "You're on the list. We'll email when access opens.",
				error: (err) => (err as Error)?.message ?? "Couldn't join the waitlist.",
			});
			joined = true;
		} finally {
			loading = false;
		}
	}
</script>

{#if joined}
	<div class="flex flex-col items-center gap-3 text-center" in:fly={{ y: 8, duration: 360, easing: cubicOut }}>
		<span class="glass-chip grid size-11 place-items-center rounded-xl text-primary">
			<MailCheck class="size-5" />
		</span>
		<div>
			<h2 class="text-sm font-semibold text-foreground">You're on the list</h2>
			<p class="mt-1 text-xs text-muted-foreground">
				We'll email <span class="font-medium text-foreground">{email}</span> when your spot opens.
			</p>
		</div>
	</div>
{:else}
	<form class="flex flex-col gap-3.5" onsubmit={submit}>
		<Label class="flex flex-col items-stretch gap-1.5">
			<span class="text-xs font-semibold text-foreground/85">Your email</span>
			<Input
				type="email"
				required
				autocomplete="email"
				bind:value={email}
				placeholder="founder@startup.com"
				class="h-10"
			/>
		</Label>
		<Button type="submit" disabled={loading} class="group/cta mt-1 w-full gap-2">
			{loading ? "Joining…" : "Join the waitlist"}
			{#if loading}
				<LoaderCircle class="size-4 animate-spin" />
			{:else}
				<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
			{/if}
		</Button>
	</form>
{/if}
