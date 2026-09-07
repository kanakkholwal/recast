<script lang="ts">
import { ArrowLeft, ArrowRight, Home, RefreshCw } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import { dev } from "$app/environment";
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { Container, Reveal } from "$lib/components";
import { ACCENT_TEXT, errorCopy, pickStatusIcon, suggestions } from "$lib/error/error-copy";

const status = $derived(page.status);
const message = $derived(page.error?.message ?? "");
const isServerError = $derived(status >= 500);

const copyFor = $derived(errorCopy(status, message, isServerError));
// The status renders as its own numeral, so drop the "404 · " prefix here.
const eyebrow = $derived(copyFor.eyebrow.replace(/^\d+\s*[·-]\s*/, ""));
const StatusIcon = $derived(pickStatusIcon(status, isServerError));
</script>

<svelte:head>
	<title>{status} - Recast</title>
	<meta name="robots" content="noindex,nofollow" />
</svelte:head>

<main class="text-foreground">
	<section class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40">
		<Container class="pb-12">
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<span
						class={cn(
							"font-display text-heading-sm leading-none tabular-nums",
							ACCENT_TEXT[copyFor.accent],
						)}
					>
						{status}
					</span>
					<span class="inline-flex items-center gap-2 text-body-sm font-medium text-muted-foreground">
						<StatusIcon class="size-4" />
						{eyebrow}
					</span>
				</div>
			</Reveal>

			<Reveal variant="up" delay={60} class="mt-10">
				<h1 class="max-w-2xl font-display text-balance text-heading-lg md:text-display">
					{copyFor.title}
				</h1>
			</Reveal>
			<Reveal variant="up" delay={120} class="mt-4">
				<p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
					{copyFor.body}
				</p>
			</Reveal>

			<Reveal variant="up" delay={180} class="mt-8 flex flex-wrap items-center gap-3">
				{#if isServerError}
					<Button onclick={() => location.reload()} variant="dark" class="gap-2">
						<RefreshCw class="size-4" />
						Try again
					</Button>
				{:else}
					<Button onclick={() => goto("/")} variant="dark" class="gap-2">
						<Home class="size-4" />
						Back home
					</Button>
				{/if}
				<Button onclick={() => history.back()} variant="outline" class="gap-2">
					<ArrowLeft class="size-4" />
					Go back
				</Button>
			</Reveal>

			<!-- Dev-only detail. Production stays clean; the error is reported instead. -->
			{#if dev && message}
				<details class="mt-8 max-w-2xl border-y border-border-low py-4">
					<summary class="cursor-pointer text-body-sm font-medium text-muted-foreground">
						Dev details
					</summary>
					<pre
						class="mt-3 overflow-x-auto whitespace-pre-wrap font-mono text-caption text-foreground"><code
							>{message}</code
						></pre>
				</details>
			{/if}
		</Container>

		<!-- Three anchored next steps. A full site map here reads like a dead end. -->
		<Container class="border-t border-border-low">
			<ul class="grid grid-cols-1 gap-px bg-border-low sm:grid-cols-3">
				{#each suggestions as item, i (item.href)}
					{@const Icon = item.icon}
					<Reveal variant="up" delay={220 + i * 70} as="li" class="bg-background">
						<a href={item.href} class="group/sug flex h-full flex-col py-6 sm:px-6">
							<Icon class="size-5 text-muted-foreground" />
							<span class="mt-4 inline-flex items-center gap-1.5 font-display text-body font-medium text-foreground">
								{item.label}
								<ArrowRight
									class="size-3.5 text-muted-foreground transition-transform duration-200 group-hover/sug:translate-x-0.5 motion-reduce:transition-none"
								/>
							</span>
							<span class="mt-1 text-body-sm text-muted-foreground">{item.desc}</span>
						</a>
					</Reveal>
				{/each}
			</ul>
		</Container>

		<Container class="border-t border-border-low">
			<p class="py-4 text-body-sm text-muted-foreground">
				Still stuck?
				<a
					href="https://github.com/kanakkholwal/recast/issues/new"
					target="_blank"
					rel="noopener noreferrer"
					class="text-foreground underline-offset-4 hover:underline"
				>
					Open an issue
				</a>
				and we will take a look.
			</p>
		</Container>
	</section>
</main>
