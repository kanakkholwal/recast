<script lang="ts">
	import { goto } from "$app/navigation";
	import Logo from "$lib/logo.svelte";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import Github from "@lucide/svelte/icons/github";
	import { ArrowRight } from "lucide-svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let email = $state("");
	let password = $state("");

	// Auth is intentionally unimplemented — this just lands you in the dashboard.
	function signIn(e: SubmitEvent) {
		e.preventDefault();
		toast.info("Auth isn't wired up — dropping you into the local dashboard.");
		goto("/dashboard");
	}
</script>

<svelte:head>
	<title>Sign in — Recast Dashboard</title>
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

	<div
		class="w-full max-w-sm"
		in:fly={{ y: 16, duration: 600, easing: cubicOut }}
	>
		<div class="flex flex-col items-center text-center">
			<a href="/" class="group/logo flex items-center gap-2.5" aria-label="Recast — home">
				<span
					class="grid size-9 place-items-center rounded-xl bg-foreground p-1 text-background shadow-craft-sm transition-transform group-hover/logo:rotate-[-4deg]"
				>
					<Logo size="22" color="transparent" fill="currentColor" />
				</span>
				<span class="text-lg font-semibold tracking-tight text-foreground">Recast</span>
			</a>
			<h1 class="mt-7 text-2xl font-semibold tracking-tight text-foreground">
				Sign in to your dashboard
			</h1>
			<p class="mt-1.5 text-sm text-muted-foreground">
				Manage your recordings, storage, and integrations.
			</p>
		</div>

		<div class="glass-card mt-8 rounded-2xl p-6 shadow-craft-lg sm:p-7">
			<Button
				variant="outline"
				class="w-full gap-2"
				onclick={() => {
					toast.info("OAuth isn't wired up — local-dev placeholder.");
					goto("/dashboard");
				}}
			>
				<Github class="size-4" />
				Continue with GitHub
			</Button>

			<div class="my-5 flex items-center gap-3">
				<span class="h-px flex-1 bg-border-low/60"></span>
				<span class="text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
					or
				</span>
				<span class="h-px flex-1 bg-border-low/60"></span>
			</div>

			<form class="flex flex-col gap-3.5" onsubmit={signIn}>
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-semibold text-foreground/85">Email</span>
					<input
						type="email"
						required
						bind:value={email}
						placeholder="you@startup.com"
						class="rounded-lg border border-border-low/70 bg-background/80 px-3 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/60 focus:border-primary/60"
					/>
				</label>
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-semibold text-foreground/85">Password</span>
					<input
						type="password"
						required
						bind:value={password}
						placeholder="••••••••"
						class="rounded-lg border border-border-low/70 bg-background/80 px-3 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/60 focus:border-primary/60"
					/>
				</label>
				<Button type="submit" class="group/cta mt-1 w-full gap-2">
					Sign in
					<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
				</Button>
			</form>
		</div>

		<p class="mt-6 text-center text-xs text-muted-foreground">
			Local development only · authentication is not yet implemented.
		</p>
	</div>
</div>
