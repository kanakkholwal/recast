<script lang="ts">
	import { dev } from "$app/environment";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { Button } from "@recast/ui/button";
	import {
		ArrowLeft,
		BookOpen,
		Compass,
		Home,
		LifeBuoy,
		MonitorPlay,
		RefreshCw,
		ScrollText,
		Search,
	} from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fade, fly } from "svelte/transition";

	const status = $derived(page.status);
	const message = $derived(page.error?.message ?? "");
	const isServerError = $derived(status >= 500);

	/**
	 * One copy block per status code we want a custom face for. Anything we
	 * don't have an entry for falls back to `default` so the page always
	 * renders something sensible — even for a 418.
	 */
	type Copy = {
		eyebrow: string;
		title: string;
		body: string;
		accent: "primary" | "amber" | "destructive";
	};
	const copyFor = $derived<Copy>(
		({
			404: {
				eyebrow: "404 · Lost in the timeline",
				title: "We can't find that frame.",
				body:
					"The link is broken, the page moved, or the URL has a typo. Nothing on your end — let's get you back to something useful.",
				accent: "primary",
			},
			403: {
				eyebrow: "403 · Locked",
				title: "Not yours to see.",
				body:
					"You're signed in, but this corner isn't open to your account. If you think that's a mistake, ping support.",
				accent: "amber",
			},
			401: {
				eyebrow: "401 · Sign in first",
				title: "You'll need an account.",
				body: "This page wants a signed-in user. Sign in and we'll bring you straight back.",
				accent: "amber",
			},
			500: {
				eyebrow: "500 · Recast tripped",
				title: "Something broke on our end.",
				body:
					"Not your code — that's on us. The error was logged. Try the page again in a moment, or head back to where you were.",
				accent: "destructive",
			},
			default: {
				eyebrow: `${status} · ${isServerError ? "Server error" : "Couldn't load"}`,
				title: "This page didn't render.",
				body:
					message ||
					"Something went sideways loading the page. Try again, or head home.",
				accent: isServerError ? "destructive" : "primary",
			},
		}[status] ?? {
			eyebrow: `${status} · Couldn't load`,
			title: "This page didn't render.",
			body: message || "Try again, or head home.",
			accent: "primary",
		}) as Copy,
	);

	// Suggestion tiles — what to try next. Keeping this curated rather than
	// site-map-y on purpose: 3 anchored next steps reads as helpful, a
	// 12-link tree reads like a dead end.
	const suggestions = [
		{ icon: Home, label: "Home", href: "/", desc: "The product overview." },
		{ icon: MonitorPlay, label: "Download", href: "/download", desc: "Get the app for your OS." },
		{ icon: BookOpen, label: "Changelog", href: "/changelog", desc: "What we shipped recently." },
	];

	const accentRing = $derived(
		({
			primary: "ring-primary/25 bg-primary/10 text-primary",
			amber:
				"ring-amber-500/30 bg-amber-500/15 text-amber-600 dark:text-amber-400",
			destructive: "ring-destructive/30 bg-destructive/12 text-destructive",
		}[copyFor.accent]),
	);

	const accentBackdrop = $derived(
		({
			primary: "color-mix(in srgb, var(--color-primary) 10%, transparent)",
			amber: "color-mix(in srgb, oklch(72% 0.18 65) 10%, transparent)",
			destructive:
				"color-mix(in srgb, var(--color-destructive) 8%, transparent)",
		}[copyFor.accent]),
	);

	const StatusIcon = $derived(
		status === 404 ? Compass : isServerError ? LifeBuoy : Search,
	);
</script>

<svelte:head>
	<title>{status} — Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<div class="relative grid min-h-[80vh] place-items-center px-6 py-16 text-foreground">
	<!-- Atmospheric accents, tinted to the status (primary / amber / destructive). -->
	<div
		aria-hidden="true"
		class="pointer-events-none absolute inset-0 -z-10"
		style="background: radial-gradient(ellipse 70% 50% at 50% 0%, {accentBackdrop}, transparent 72%);"
	></div>
	<div
		aria-hidden="true"
		class="bg-grid bg-grid-fade pointer-events-none absolute inset-0 -z-10 opacity-30"
	></div>

	<div
		class="w-full max-w-xl"
		in:fly={{ y: 20, duration: 520, easing: cubicOut }}
	>
		<div class="flex flex-col items-center text-center">
			<span
				class="glass-chip grid size-14 place-items-center rounded-2xl ring-1 {accentRing}"
				in:fade={{ duration: 360, delay: 80 }}
			>
				<StatusIcon class="size-6" />
			</span>

			<!-- The status itself, big and unmistakable — easier to skim than the title. -->
			<div class="mt-6 flex items-baseline gap-3">
				<span class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
					{copyFor.eyebrow}
				</span>
			</div>

			<h1 class="text-balance mt-3 text-3xl font-semibold leading-tight tracking-tight text-foreground sm:text-4xl">
				{copyFor.title}
			</h1>
			<p class="text-pretty mt-3 max-w-md text-sm leading-relaxed text-muted-foreground">
				{copyFor.body}
			</p>

			<div class="mt-7 flex flex-wrap items-center justify-center gap-2.5">
				<Button onclick={() => history.back()} variant="outline" class="gap-2">
					<ArrowLeft class="size-4" />
					Go back
				</Button>
				{#if isServerError}
					<Button onclick={() => location.reload()} class="gap-2">
						<RefreshCw class="size-4" />
						Try again
					</Button>
				{:else}
					<Button onclick={() => goto("/")} class="gap-2">
						<Home class="size-4" />
						Back home
					</Button>
				{/if}
			</div>

			<!-- Dev-only stack/details. Production stays clean — surface the error
			     via Sentry/PostHog (when wired) instead of leaking internals. -->
			{#if dev && message}
				<details
					class="mt-7 w-full max-w-md rounded-xl border border-border-low/50 bg-foreground/2 p-4 text-left text-xs"
				>
					<summary class="cursor-pointer font-mono text-[11px] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
						Dev details
					</summary>
					<pre class="mt-3 overflow-x-auto whitespace-pre-wrap font-mono text-[11px] leading-relaxed text-foreground/85"><code>{message}</code></pre>
				</details>
			{/if}
		</div>

		<!-- Suggestions grid — keep it short, give users an obvious next move. -->
		<div class="mt-10 grid gap-2.5 sm:grid-cols-3">
			{#each suggestions as item, i}
				{@const Icon = item.icon}
				<a
					href={item.href}
					class="group/sug glass-card flex flex-col gap-1.5 rounded-xl p-4 transition-all duration-200 hover:-translate-y-0.5 hover:shadow-craft-md"
					in:fly={{ y: 10, duration: 360, delay: 180 + i * 60, easing: cubicOut }}
				>
					<span class="glass-chip grid size-8 place-items-center rounded-lg text-foreground/70 transition-colors group-hover/sug:text-primary">
						<Icon class="size-4" />
					</span>
					<div>
						<div class="text-sm font-semibold tracking-tight text-foreground">
							{item.label}
						</div>
						<div class="mt-0.5 text-[11px] leading-relaxed text-muted-foreground">
							{item.desc}
						</div>
					</div>
				</a>
			{/each}
		</div>

		<p class="mt-8 text-center text-[11px] text-muted-foreground">
			Still stuck?
			<a
				href="https://github.com/kanakkholwal/recast/issues/new"
				target="_blank"
				rel="noopener noreferrer"
				class="inline-flex items-center gap-1 font-semibold text-foreground transition-colors hover:text-primary"
			>
				<ScrollText class="size-3" />
				Open an issue
			</a>
			and we'll take a look.
		</p>
	</div>
</div>
