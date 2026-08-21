<script lang="ts">
import { dev } from "$app/environment";
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { Button } from "@recast/ui/button";
import { ArrowLeft, Home, RefreshCw, ScrollText } from "@recast/icons";
import { cubicOut } from "svelte/easing";
import { fade, fly } from "svelte/transition";
import {
	ACCENT_BACKDROP,
	ACCENT_RING,
	errorCopy,
	pickStatusIcon,
	suggestions,
} from "$lib/error/error-copy";

const status = $derived(page.status);
const message = $derived(page.error?.message ?? "");
const isServerError = $derived(status >= 500);

const copyFor = $derived(errorCopy(status, message, isServerError));
const accentRing = $derived(ACCENT_RING[copyFor.accent]);
const accentBackdrop = $derived(ACCENT_BACKDROP[copyFor.accent]);
const StatusIcon = $derived(pickStatusIcon(status, isServerError));
</script>

<svelte:head>
	<title>{status} - Recast</title>
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
				class="pill grid size-14 place-items-center rounded-2xl ring-1 {accentRing}"
				in:fade={{ duration: 360, delay: 80 }}
			>
				<StatusIcon class="size-6" />
			</span>

			<!-- The status itself, big and unmistakable — easier to skim than the title. -->
			<div class="mt-6 flex items-baseline gap-3">
				<span class="text-body-sm font-medium text-muted-foreground">
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
					class="mt-7 w-full max-w-md rounded-xl border border-border-low bg-paper p-4 text-left text-xs"
				>
					<summary class="cursor-pointer font-mono text-body-sm font-medium text-muted-foreground">
						Dev details
					</summary>
					<pre class="mt-3 overflow-x-auto whitespace-pre-wrap font-mono text-caption leading-relaxed text-foreground"><code>{message}</code></pre>
				</details>
			{/if}
		</div>

		<!-- Suggestions grid — keep it short, give users an obvious next move. -->
		<div class="mt-10 grid gap-2.5 sm:grid-cols-3">
			{#each suggestions as item, i}
				{@const Icon = item.icon}
				<a
					href={item.href}
					class="group/sug surface-lg flex flex-col gap-1.5 rounded-xl p-4 transition-all duration-200 hover:"
					in:fly={{ y: 10, duration: 360, delay: 180 + i * 60, easing: cubicOut }}
				>
					<span class="pill grid size-8 place-items-center rounded-lg text-muted-foreground transition-colors group-hover/sug:text-primary">
						<Icon class="size-4" />
					</span>
					<div>
						<div class="text-sm font-semibold tracking-tight text-foreground">
							{item.label}
						</div>
						<div class="mt-0.5 text-caption leading-relaxed text-muted-foreground">
							{item.desc}
						</div>
					</div>
				</a>
			{/each}
		</div>

		<p class="mt-8 text-center text-caption text-muted-foreground">
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
