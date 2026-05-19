<script lang="ts">
	import {
		Container,
		Eyebrow,
		Footer,
		Reveal,
		Section,
		SectionHeader,
	} from "$lib/components";
	import { Button } from "@recast/ui/button";
	import {
		ArrowRight,
		Check,
		Cloud,
		Download,
		Minus,
		Sparkles,
	} from "lucide-svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let email = $state("");
	let joined = $state(false);
	function joinWaitlist(e: SubmitEvent) {
		e.preventDefault();
		if (email.trim()) joined = true;
	}

	type Cell = boolean | string;
	const rows: { label: string; free: Cell; cloud: Cell }[] = [
		{ label: "Record, auto-polish & edit", free: true, cloud: true },
		{ label: "Hardware-accelerated export", free: true, cloud: true },
		{ label: "Local .recast project files", free: true, cloud: true },
		{ label: "Fully offline — no account", free: true, cloud: true },
		{ label: "Shareable hosted links", free: "10 active", cloud: "Unlimited" },
		{ label: "Watch analytics", free: false, cloud: true },
		{ label: "Custom branding & domain", free: false, cloud: true },
		{ label: "Password & link expiry", free: false, cloud: true },
		{ label: "Cloud library & device sync", free: false, cloud: true },
		{ label: "Recast watermark on links", free: "Always on", cloud: "Removable" },
	];
</script>

<svelte:head>
	<title>Pricing — Recast</title>
	<meta
		name="description"
		content="The Recast app is free forever. Recast Cloud adds hosted demo links, watch analytics, and custom branding — join the waitlist for early access."
	/>
</svelte:head>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-20">
		<Container>
			<div class="mx-auto flex max-w-3xl flex-col items-center gap-7 text-center">
				<Eyebrow icon={Sparkles} variant="primary">Pricing</Eyebrow>
				<h1 class="text-balance text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl">
					The app is free.
					<span class="block font-medium italic text-foreground/40">You pay to share.</span>
				</h1>
				<p class="text-pretty max-w-2xl text-base leading-relaxed text-muted-foreground sm:text-lg">
					Recording and polishing your demos costs nothing — and always will. Recast Cloud is
					the optional layer for sending them as links with analytics.
				</p>
			</div>
		</Container>
	</Section>

	<!-- Plan cards -->
	<Section spacing="tight">
		<Container>
			<div class="grid gap-4 lg:grid-cols-2">
				<!-- Free -->
				<Reveal variant="left">
					<article class="glass-card flex h-full flex-col rounded-2xl p-8 sm:p-10">
						<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
							Recast app
						</span>
						<div class="mt-3 flex items-baseline gap-2">
							<span class="text-5xl font-semibold tracking-tight text-foreground">$0</span>
							<span class="text-sm text-muted-foreground">free forever</span>
						</div>
						<p class="mt-4 text-sm leading-relaxed text-muted-foreground">
							The complete recorder and editor. Offline, no account, no asterisk.
						</p>
						<ul class="mt-7 space-y-3">
							{#each ["Record region, window, or full screen", "Auto cursor smoothing, layouts & zoom", "Trim, edit & hardware-accelerated export", "10 shareable links included", "macOS, Windows & Linux"] as point}
								<li class="flex items-start gap-2.5 text-sm text-foreground/85">
									<Check class="mt-0.5 size-4 shrink-0 text-primary" />
									{point}
								</li>
							{/each}
						</ul>
						<div class="mt-8 pt-2">
							<Button href="/download" size="lg" class="w-full gap-2">
								<Download class="size-4" />
								Download free
							</Button>
						</div>
					</article>
				</Reveal>

				<!-- Cloud -->
				<Reveal variant="right" delay={80}>
					<article class="glass-card relative flex h-full flex-col overflow-hidden rounded-2xl p-8 ring-1 ring-primary/25 sm:p-10">
						<div
							aria-hidden="true"
							class="pointer-events-none absolute -right-12 -top-12 size-56 rounded-full bg-primary/10 blur-3xl"
						></div>
						<div class="relative flex items-center justify-between">
							<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-primary">
								Recast Cloud
							</span>
							<span class="glass-chip inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-foreground/80">
								<Cloud class="size-3 text-primary" />
								Coming soon
							</span>
						</div>
						<div class="relative mt-3 flex items-baseline gap-2">
							<span class="text-5xl font-semibold tracking-tight text-foreground">Soon</span>
							<span class="text-sm text-muted-foreground">pricing in progress</span>
						</div>
						<p class="relative mt-4 text-sm leading-relaxed text-muted-foreground">
							Everything in the free app, plus the sharing layer for demos that need to travel.
						</p>
						<ul class="relative mt-7 space-y-3">
							{#each ["Unlimited hosted demo links", "Watch analytics — who watched, how far", "Custom branding & your own domain", "Password protection & link expiry", "Cloud library, synced across machines"] as point}
								<li class="flex items-start gap-2.5 text-sm text-foreground/85">
									<Check class="mt-0.5 size-4 shrink-0 text-primary" />
									{point}
								</li>
							{/each}
						</ul>

						<div class="relative mt-8 pt-2">
							{#if joined}
								<div
									class="flex items-center gap-3 rounded-xl border border-primary/30 bg-primary/8 px-4 py-3.5"
									in:fly={{ y: 8, duration: 400, easing: cubicOut }}
								>
									<span class="grid size-7 place-items-center rounded-full bg-primary/15 text-primary">
										<Check class="size-4" />
									</span>
									<span class="text-sm font-medium text-foreground">
										You're on the early-access list.
									</span>
								</div>
							{:else}
								<form class="flex flex-col gap-2.5 sm:flex-row" onsubmit={joinWaitlist}>
									<input
										type="email"
										required
										bind:value={email}
										placeholder="founder@startup.com"
										class="flex-1 rounded-lg border border-border-low/70 bg-background/80 px-3.5 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
									/>
									<Button type="submit" size="lg" class="gap-2">
										Join waitlist
										<ArrowRight class="size-4" />
									</Button>
								</form>
							{/if}
						</div>
					</article>
				</Reveal>
			</div>
		</Container>
	</Section>

	<!-- Comparison table -->
	<Section class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Side by side"
				title="What you get, where."
				description="The app does the work. Cloud carries it the last mile."
				align="center"
			/>

			<Reveal variant="blur" class="mt-14">
				<div class="overflow-hidden rounded-2xl border border-border-low/50">
					<div class="grid grid-cols-[1.6fr_1fr_1fr] border-b border-border-low/50 bg-foreground/2 text-[11px] font-semibold uppercase tracking-[0.16em]">
						<div class="px-5 py-3.5 text-muted-foreground">Feature</div>
						<div class="border-l border-border-low/50 px-5 py-3.5 text-center text-foreground">Free app</div>
						<div class="border-l border-border-low/50 px-5 py-3.5 text-center text-primary">Cloud</div>
					</div>
					{#each rows as row, i}
						<div class="grid grid-cols-[1.6fr_1fr_1fr] {i < rows.length - 1 ? 'border-b border-border-low/40' : ''}">
							<div class="px-5 py-3.5 text-sm text-foreground/85">{row.label}</div>
							{#each [row.free, row.cloud] as cell}
								<div class="flex items-center justify-center border-l border-border-low/40 px-5 py-3.5 text-center text-sm">
									{#if cell === true}
										<Check class="size-4 text-primary" />
									{:else if cell === false}
										<Minus class="size-4 text-muted-foreground/40" />
									{:else}
										<span class="text-xs font-medium text-foreground/80">{cell}</span>
									{/if}
								</div>
							{/each}
						</div>
					{/each}
				</div>
			</Reveal>

			<Reveal variant="up" class="mt-8">
				<p class="text-center text-xs text-muted-foreground">
					Cloud pricing isn't final. The free tier — 10 active shareable links — stays free
					forever, no card required.
				</p>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
