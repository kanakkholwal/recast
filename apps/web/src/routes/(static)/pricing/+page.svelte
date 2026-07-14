<script lang="ts">
	import {
		Container,
		Eyebrow,
		Footer,
		HeroBackdrop,
		Reveal,
		Section,
		SectionHeader,
		SeoMeta,
	} from "$lib/components";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import {
		ArrowRight,
		Building2,
		Check,
		Cloud,
		Download,
		HardDriveUpload,
		LoaderCircle,
		Mail,
		Minus,
		ShieldCheck,
		Tag,
		Users,
	} from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	let email = $state("");
	let joined = $state(false);
	let loading = $state(false);
	async function joinWaitlist(e: SubmitEvent) {
		e.preventDefault();
		if (!email.trim() || loading) return;
		loading = true;
		try {
			await toast.promise(
				(async () => {
					const res = await fetch("/api/waitlist", {
						method: "POST",
						headers: { "Content-Type": "application/json" },
						body: JSON.stringify({ email, source: "pricing" }),
					});
					const data = (await res.json().catch(() => ({}))) as {
						ok?: boolean;
						error?: string;
					};
					if (!data.ok) throw new Error(data.error ?? "Couldn't join the waitlist.");
				})(),
				{
					loading: "Adding you to the waitlist…",
					success: "You're on the list. We'll email when access opens.",
					error: (err) => (err as Error)?.message ?? "Couldn't join the waitlist.",
				},
			);
			joined = true;
		} finally {
			loading = false;
		}
	}

	type Cell = boolean | string;
	type Row = { label: string; desktop: Cell; cloudFree: Cell; cloudPro: Cell; enterprise: Cell };
	type RowGroup = { heading: string; rows: Row[] };

	// Comparison table. Storage row is the centerpiece of the new positioning:
	// Drive ships in the free desktop today; additional BYO destinations land
	// on the Cloud free tier; paid Pro adds Recast-managed plus custom buckets.
	const groups: RowGroup[] = [
		{
			heading: "Desktop app",
			rows: [
				{ label: "Record, auto-polish, edit, export", desktop: true, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Recording profiles", desktop: true, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Pause and resume mid-take", desktop: true, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Annotations, blur, camera bubble", desktop: true, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Hardware-accelerated export", desktop: true, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Local .recast project files", desktop: true, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Account required to record", desktop: "Never", cloudFree: "Never", cloudPro: "Never", enterprise: "Never" },
			],
		},
		{
			heading: "Sharing",
			rows: [
				{ label: "Share link from your storage", desktop: true, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Hosted Recast player page", desktop: false, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Watch analytics", desktop: false, cloudFree: "Basic", cloudPro: "Full", enterprise: "Full + export" },
				{ label: "Password protection and link expiry", desktop: false, cloudFree: false, cloudPro: true, enterprise: true },
				{ label: "Per-viewer access controls", desktop: false, cloudFree: false, cloudPro: true, enterprise: true },
				{ label: "Custom branding and domain", desktop: false, cloudFree: false, cloudPro: true, enterprise: true },
			],
		},
		{
			heading: "Storage",
			rows: [
				{ label: "Google Drive (your account)", desktop: true, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Cloudinary, autorender.io", desktop: false, cloudFree: "Planned", cloudPro: "Planned", enterprise: "Planned" },
				{ label: "Recast-managed storage", desktop: false, cloudFree: false, cloudPro: true, enterprise: true },
				{ label: "Custom S3 / R2 / Azure / GCP", desktop: false, cloudFree: false, cloudPro: true, enterprise: true },
				{ label: "Data residency control", desktop: false, cloudFree: false, cloudPro: true, enterprise: true },
			],
		},
		{
			heading: "Team and admin",
			rows: [
				{ label: "Team workspaces", desktop: false, cloudFree: "Up to 3", cloudPro: "Up to 50", enterprise: "Unlimited" },
				{ label: "Roles (owner, admin, member)", desktop: false, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Email invitations", desktop: false, cloudFree: true, cloudPro: true, enterprise: true },
				{ label: "Audit log", desktop: false, cloudFree: false, cloudPro: false, enterprise: true },
				{ label: "SSO / SAML / SCIM", desktop: false, cloudFree: false, cloudPro: false, enterprise: true },
				{ label: "Dedicated success and SLAs", desktop: false, cloudFree: false, cloudPro: false, enterprise: true },
			],
		},
	];

	type ColKey = "desktop" | "cloudFree" | "cloudPro" | "enterprise";
	const columns: { key: ColKey; label: string; tone: "muted" | "primary" | "foreground" }[] = [
		{ key: "desktop", label: "Desktop", tone: "foreground" },
		{ key: "cloudFree", label: "Cloud Free", tone: "muted" },
		{ key: "cloudPro", label: "Cloud Pro", tone: "primary" },
		{ key: "enterprise", label: "Enterprise", tone: "foreground" },
	];
</script>

<SeoMeta
	title="Free, local, yours."
	description="Recast Desktop is free forever and runs offline. Recast Cloud adds hosted sharing and watch analytics with bring-your-own storage, free or paid."
	eyebrow="Pricing"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-20">
		<HeroBackdrop src="/background-footer.webp" />
		<Container>
			<div class="mx-auto flex max-w-3xl flex-col items-center gap-7 text-center">
				<Eyebrow variant="primary">Pricing</Eyebrow>
				<h1 class="text-balance text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5rem]">
					Free, local,
					<span class="block font-medium italic text-foreground/40">yours.</span>
				</h1>
				<p class="text-pretty max-w-2xl text-base leading-relaxed text-muted-foreground sm:text-lg">
					The desktop app is free forever and runs offline. Recast Cloud, when it lands, is a sharing layer on top. Bring your own storage on the free tier, or let Recast manage it (or plug in your own bucket) on Pro.
				</p>
				<div class="mt-2 inline-flex flex-wrap items-center justify-center gap-2 text-[11.5px] font-medium text-foreground/75">
					<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 bg-card/40 px-3 py-1 ring-1 ring-inset ring-border-low/30">
						<ShieldCheck class="size-3.5 text-primary" /> No telemetry
					</span>
					<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 bg-card/40 px-3 py-1 ring-1 ring-inset ring-border-low/30">
						<HardDriveUpload class="size-3.5 text-primary" /> Bring your own storage
					</span>
					<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 bg-card/40 px-3 py-1 ring-1 ring-inset ring-border-low/30">
						<Tag class="size-3.5 text-primary" /> No per-seat tax
					</span>
				</div>
			</div>
		</Container>
	</Section>

	<!-- Plan cards: Desktop (today), Cloud Free (waitlist), Cloud Pro (waitlist) -->
	<Section spacing="tight">
		<Container>
			<div class="grid gap-4 lg:grid-cols-3">
				<!-- Desktop (free, today) -->
				<Reveal variant="left">
					<article class="glass-card flex h-full flex-col rounded-2xl p-7 sm:p-8">
						<div class="flex items-center justify-between">
							<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
								Desktop
							</span>
							<span class="inline-flex items-center gap-1.5 rounded-full bg-emerald-500/12 px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-emerald-600 ring-1 ring-inset ring-emerald-500/25 dark:text-emerald-400">
								<span class="size-1.5 rounded-full bg-emerald-500"></span>
								Available now
							</span>
						</div>
						<div class="mt-3 flex items-baseline gap-2">
							<span class="text-5xl font-semibold tracking-tight text-foreground">$0</span>
							<span class="text-sm text-muted-foreground">forever</span>
						</div>
						<p class="mt-4 text-sm leading-relaxed text-muted-foreground">
							The whole recorder and editor, offline. No account, no telemetry, GPLv3 open source.
						</p>
						<ul class="mt-6 space-y-3">
							{#each [
								"Record region, window, or full screen",
								"Recording profiles and pause / resume",
								"Smart zoom, cursor smoothing, silence cuts",
								"Annotations, blur, camera bubble",
								"Upload exports to your own Google Drive",
								"Windows stable, macOS and Linux in beta",
							] as point}
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

				<!-- Cloud Free (waitlist) -->
				<Reveal variant="up" delay={80}>
					<article class="glass-card relative flex h-full flex-col overflow-hidden rounded-2xl p-7 ring-1 ring-primary/25 sm:p-8">
						<div
							aria-hidden="true"
							class="pointer-events-none absolute -right-12 -top-12 size-56 rounded-full bg-primary/10 blur-3xl"
						></div>
						<div class="relative flex items-center justify-between">
							<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-primary">
								Cloud Pro
							</span>
							<span class="glass-chip inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-foreground/80">
								<Cloud class="size-3 text-primary" />
								Coming next
							</span>
						</div>
						<div class="relative mt-3 flex items-baseline gap-2">
							<span class="text-5xl font-semibold tracking-tight text-foreground">Soon</span>
							<span class="text-sm text-muted-foreground">~$10 / mo</span>
						</div>
						<p class="relative mt-4 text-sm leading-relaxed text-muted-foreground">
							Hosted sharing with watch analytics, custom branding, and team workspaces. Storage stays yours (Recast-managed or your own bucket).
						</p>
						<div class="relative mt-5 inline-flex items-center gap-2 self-start rounded-full border border-primary/30 bg-primary/8 px-3 py-1 text-[11px] font-medium text-foreground/90">
							<Users class="size-3.5 text-primary" />
							Up to 50 workspace members
						</div>
						<ul class="relative mt-6 space-y-3">
							{#each [
								"Everything in Desktop, plus a hosted player page",
								"Full watch analytics (who watched, how far)",
								"Password protection and link expiry",
								"Per-viewer access controls",
								"Custom branding and your own domain",
								"Recast-managed storage or your own S3, R2, Azure, GCP bucket",
							] as point}
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
								<form class="flex flex-col gap-2.5" onsubmit={joinWaitlist}>
									<input
										type="email"
										required
										bind:value={email}
										placeholder="founder@startup.com"
										class="w-full rounded-lg border border-border-low/70 bg-background/80 px-3.5 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
									/>
									<Button type="submit" size="lg" disabled={loading} class="gap-2">
										{loading ? "Joining…" : "Join Cloud waitlist"}
										{#if loading}
											<LoaderCircle class="size-4 animate-spin" />
										{:else}
											<ArrowRight class="size-4" />
										{/if}
									</Button>
								</form>
							{/if}
						</div>
					</article>
				</Reveal>

				<!-- Cloud Free tier (also waitlist; storage-agnostic flavour) -->
				<Reveal variant="right" delay={160}>
					<article class="glass-card flex h-full flex-col rounded-2xl p-7 sm:p-8">
						<div class="flex items-center justify-between">
							<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
								Cloud Free
							</span>
							<span class="glass-chip inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-foreground/70">
								<Cloud class="size-3" />
								Coming next
							</span>
						</div>
						<div class="mt-3 flex items-baseline gap-2">
							<span class="text-5xl font-semibold tracking-tight text-foreground">$0</span>
							<span class="text-sm text-muted-foreground">BYO storage</span>
						</div>
						<p class="mt-4 text-sm leading-relaxed text-muted-foreground">
							A sharing layer on top of the storage you already pay for. Free for small teams, no card required.
						</p>
						<div class="mt-5 inline-flex items-center gap-2 self-start rounded-full border border-border-low/60 bg-foreground/3 px-3 py-1 text-[11px] font-medium text-foreground/80">
							<HardDriveUpload class="size-3.5 text-primary" />
							Bring your own bucket
						</div>
						<ul class="mt-6 space-y-3">
							{#each [
								"Hosted Recast player page for each upload",
								"Basic watch analytics",
								"Bring your own storage (Google Drive today)",
								"Cloudinary and autorender.io planned",
								"Up to 3 workspace members",
								"Upgrade to Pro any time, your storage carries over",
							] as point}
								<li class="flex items-start gap-2.5 text-sm text-foreground/85">
									<Check class="mt-0.5 size-4 shrink-0 text-primary" />
									{point}
								</li>
							{/each}
						</ul>
						<div class="mt-8 pt-2">
							<Button href="/waitlist" variant="secondary" size="lg" class="w-full gap-2">
								<Cloud class="size-4" />
								Join Cloud waitlist
							</Button>
						</div>
					</article>
				</Reveal>
			</div>

			<!-- Enterprise: its own row so it can breathe and doesn't pretend to
			     be a self-serve checkout. -->
			<Reveal variant="up" delay={240} class="mt-4">
				<article class="glass-card flex flex-col gap-6 rounded-2xl p-7 sm:p-8 md:flex-row md:items-center md:gap-10">
					<div class="md:flex-1">
						<div class="flex items-center justify-between md:justify-start md:gap-4">
							<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
								Enterprise
							</span>
							<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-foreground/70">
								<Building2 class="size-3" />
								Talk to us
							</span>
						</div>
						<h3 class="mt-3 text-2xl font-semibold tracking-tight text-foreground sm:text-3xl">
							SSO, audit, and seats without a ceiling.
						</h3>
						<p class="mt-3 text-sm leading-relaxed text-muted-foreground sm:text-base">
							For larger orgs that need single sign-on, audit trails, data-residency guarantees, and a dedicated success manager. Provisioned per agreement, not self-serve.
						</p>
					</div>
					<ul class="grid grid-cols-1 gap-2.5 sm:grid-cols-2 md:max-w-md md:flex-1">
						{#each [
							"Everything in Cloud Pro",
							"SSO / SAML and SCIM provisioning",
							"Audit log and access controls",
							"Custom S3, R2, Azure, GCP buckets",
							"Dedicated success manager and SLAs",
							"Volume pricing",
						] as point}
							<li class="flex items-start gap-2 text-sm text-foreground/85">
								<Check class="mt-0.5 size-4 shrink-0 text-primary" />
								{point}
							</li>
						{/each}
					</ul>
					<div class="md:shrink-0">
						<Button
							href="mailto:hello@recast.li?subject=Recast%20Enterprise"
							variant="secondary"
							size="lg"
							class="w-full gap-2 md:w-auto"
						>
							<Mail class="size-4" />
							Contact sales
						</Button>
					</div>
				</article>
			</Reveal>
		</Container>
	</Section>

	<!-- Comparison table -->
	<Section class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Side by side"
				title="What you get, where."
				description="The desktop app does the work today. Cloud adds the sharing surface on top, with storage you can swap out."
				align="center"
			/>

			<Reveal variant="blur" class="mt-14">
				<div class="overflow-x-auto rounded-2xl border border-border-low/50">
					<div class="min-w-190">
						<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr_1fr] border-b border-border-low/50 bg-foreground/2 text-[11px] font-semibold uppercase tracking-[0.16em]">
							<div class="px-5 py-3.5 text-muted-foreground">Feature</div>
							{#each columns as col}
								<div
									class="border-l border-border-low/50 px-5 py-3.5 text-center {col.tone === 'primary' ? 'text-primary' : col.tone === 'muted' ? 'text-muted-foreground' : 'text-foreground'}"
								>
									{col.label}
								</div>
							{/each}
						</div>
						{#each groups as group, gi}
							<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr_1fr] border-b border-border-low/50 bg-foreground/1.5">
								<div class="col-span-5 px-5 py-2.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground/80">
									{group.heading}
								</div>
							</div>
							{#each group.rows as row, ri}
								{@const isLast = gi === groups.length - 1 && ri === group.rows.length - 1}
								<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr_1fr] {isLast ? '' : 'border-b border-border-low/40'}">
									<div class="px-5 py-3.5 text-sm text-foreground/85">{row.label}</div>
									{#each columns as col}
										{@const cell = row[col.key]}
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
						{/each}
					</div>
				</div>
			</Reveal>

			<Reveal variant="up" class="mt-8">
				<p class="mx-auto max-w-2xl text-balance text-center text-xs leading-relaxed text-muted-foreground">
					Cloud pricing isn't final. The desktop app stays free forever, no card required. Cloud Free will stay free for small teams that bring their own storage.
					<a href="mailto:hello@recast.li?subject=Recast%20Enterprise" class="text-foreground underline-offset-2 hover:underline">Talk to us</a> for Enterprise.
				</p>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
