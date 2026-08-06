<script lang="ts">
import { goto } from "$app/navigation";
import {
	Container,
	Footer,
	HeroBackdrop,
	Reveal,
	Section,
	SectionHeader,
	SeoMeta,
} from "$lib/components";
import { prefersReducedMotion } from "$lib/motion-core";
import { PLANS } from "$lib/billing/catalog";
import {
	ArrowRight,
	Building2,
	Check,
	Cloud,
	Download,
	HardDriveUpload,
	Mail,
	Minus,
	ShieldCheck,
	Tag,
	Users,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cubicOut } from "svelte/easing";
import { fly } from "svelte/transition";
import { extraSeatPrice, formatUsd, gb, LOOM, proPrice, teamComparison } from "./pricing.logic";

// Hero entrance: same 80ms stagger as the rest of the public pages.
const reduced = $derived(prefersReducedMotion());
const heroStagger = 80;
const riseM = (delay: number) =>
	reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };

let annual = $state(false);
const pro = $derived(proPrice(annual));
const extraSeat = $derived(extraSeatPrice(annual));
const teams = $derived(teamComparison(annual));

// Upgrading is a workspace action, so Pro can't be bought from here — the
// CTA creates the account and Polar checkout runs from settings/billing.
let email = $state("");
const signupHref = $derived(
	email.trim()
		? `/signup?email=${encodeURIComponent(email.trim())}&source=pricing`
		: "/signup?source=pricing",
);
function startWithEmail(e: SubmitEvent) {
	e.preventDefault();
	goto(signupHref);
}

type Cell = boolean | string;
type Row = { label: string; desktop: Cell; cloudFree: Cell; cloudPro: Cell; enterprise: Cell };
type RowGroup = { heading: string; rows: Row[] };

const free = PLANS.free;
const proPlan = PLANS.pro;

const groups: RowGroup[] = [
	{
		heading: "Desktop app",
		rows: [
			{
				label: "Record, auto-polish, edit, export",
				desktop: true,
				cloudFree: true,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Smart zoom, cursor smoothing, silence cuts",
				desktop: true,
				cloudFree: true,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Annotations, blur, camera bubble",
				desktop: true,
				cloudFree: true,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Hardware-accelerated export",
				desktop: true,
				cloudFree: true,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Account required to record",
				desktop: "Never",
				cloudFree: "Never",
				cloudPro: "Never",
				enterprise: "Never",
			},
		],
	},
	{
		heading: "Limits",
		rows: [
			{
				label: "Creators",
				desktop: "No account",
				cloudFree: `${free.seats.included}`,
				cloudPro: `${proPlan.seats.included} included, then ${formatUsd(proPlan.seats.monthlyUsd)}`,
				enterprise: `Up to ${PLANS.enterprise.seats.max}`,
			},
			{
				label: "Active share links",
				desktop: "—",
				cloudFree: `${free.limits.activeRecasts}`,
				cloudPro: `${proPlan.limits.activeRecasts}`,
				enterprise: "By agreement",
			},
			{
				label: "Hosted storage",
				desktop: "—",
				cloudFree: gb(free.limits.storageBytes),
				cloudPro: gb(proPlan.limits.storageBytes),
				enterprise: "By agreement",
			},
			{
				label: "Monthly delivery to viewers",
				desktop: "—",
				cloudFree: gb(free.limits.deliveryBytesPerMonth),
				cloudPro: gb(proPlan.limits.deliveryBytesPerMonth),
				enterprise: "By agreement",
			},
			{
				label: "Recording length",
				desktop: "No limit",
				cloudFree: "10 min",
				cloudPro: "4 hours",
				enterprise: "8 hours",
			},
			{
				label: "Playback quality",
				desktop: "Source",
				cloudFree: "720p",
				cloudPro: "4K",
				enterprise: "4K",
			},
		],
	},
	{
		heading: "Sharing",
		rows: [
			{
				label: "Hosted Recast player page",
				desktop: false,
				cloudFree: true,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Watch analytics",
				desktop: false,
				cloudFree: "Basic",
				cloudPro: "Full",
				enterprise: "Full + export",
			},
			{
				label: "Password protection and link expiry",
				desktop: false,
				cloudFree: false,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Per-viewer access controls",
				desktop: false,
				cloudFree: false,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Custom branding and domain",
				desktop: false,
				cloudFree: false,
				cloudPro: true,
				enterprise: true,
			},
		],
	},
	{
		heading: "Storage",
		rows: [
			{
				label: "Bring your own bucket",
				desktop: true,
				cloudFree: true,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Recast-managed storage",
				desktop: false,
				cloudFree: true,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Custom S3 / R2 / Azure / GCP",
				desktop: false,
				cloudFree: false,
				cloudPro: true,
				enterprise: true,
			},
			{
				label: "Data residency control",
				desktop: false,
				cloudFree: false,
				cloudPro: false,
				enterprise: true,
			},
		],
	},
	{
		heading: "Team and admin",
		rows: [
			{
				label: "Roles (owner, admin, member)",
				desktop: false,
				cloudFree: true,
				cloudPro: true,
				enterprise: true,
			},
			{ label: "Audit log", desktop: false, cloudFree: false, cloudPro: false, enterprise: true },
			{
				label: "SSO / SAML / SCIM",
				desktop: false,
				cloudFree: false,
				cloudPro: false,
				enterprise: true,
			},
			{
				label: "Dedicated success and SLAs",
				desktop: false,
				cloudFree: false,
				cloudPro: false,
				enterprise: true,
			},
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
	title="Pricing without the per-seat tax"
	description="Recast Desktop is free forever and runs offline. Recast Cloud Pro is $12/mo for your first three creators — Loom charges $18 each."
	eyebrow="Pricing"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-20">
		<HeroBackdrop src="/background-pricing.webp" tone="subtle" />
		<Container class="relative">
			<div class="relative z-10 mx-auto flex max-w-3xl flex-col items-center gap-7 text-center">
				<span
					in:fly={riseM(heroStagger * 0)}
					class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70"
				>
					<span class="size-1.5 rounded-full bg-primary"></span>
					Pricing
				</span>
				<h1
					in:fly={riseM(heroStagger * 1)}
					class="text-balance text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5rem]"
				>
					No per-seat
					<span class="block font-medium italic text-foreground/40">tax.</span>
				</h1>
				<p
					in:fly={riseM(heroStagger * 2)}
					class="text-pretty max-w-2xl text-base leading-relaxed text-muted-foreground sm:text-lg"
				>
					The desktop recorder and editor is free forever and runs offline. Cloud adds hosted
					sharing for {formatUsd(pro)} a month — covering your first {proPlan.seats.included}
					creators, not one.
				</p>
				<div class="mt-2 inline-flex flex-wrap items-center justify-center gap-2 text-[11.5px] font-medium text-foreground/75">
					<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 bg-card/40 px-3 py-1 ring-1 ring-inset ring-border-low/30">
						<ShieldCheck class="size-3.5 text-foreground" /> No telemetry
					</span>
					<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 bg-card/40 px-3 py-1 ring-1 ring-inset ring-border-low/30">
						<HardDriveUpload class="size-3.5 text-foreground" /> Bring your own storage
					</span>
					<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 bg-card/40 px-3 py-1 ring-1 ring-inset ring-border-low/30">
						<Tag class="size-3.5 text-foreground" /> No card to start
					</span>
				</div>

				<div
					class="mt-2 inline-flex items-center gap-1 rounded-full border border-border-low/60 bg-card/50 p-1"
					role="group"
					aria-label="Billing period"
				>
					<button
						type="button"
						aria-pressed={!annual}
						onclick={() => (annual = false)}
						class="rounded-full px-4 py-1.5 text-xs font-semibold transition-colors {annual
							? 'text-muted-foreground hover:text-foreground'
							: 'bg-foreground text-background'}"
					>
						Monthly
					</button>
					<button
						type="button"
						aria-pressed={annual}
						onclick={() => (annual = true)}
						class="rounded-full px-4 py-1.5 text-xs font-semibold transition-colors {annual
							? 'bg-foreground text-background'
							: 'text-muted-foreground hover:text-foreground'}"
					>
						Annual
						<span class="ml-1 text-[10px] font-bold text-primary">−17%</span>
					</button>
				</div>
			</div>
		</Container>
	</Section>

	<!-- Plan cards: Free (hosted or BYO), Pro (featured), Enterprise -->
	<Section spacing="tight">
		<Container>
			<div class="grid gap-4 lg:grid-cols-3">
				<!-- Cloud Free -->
				<Reveal variant="left" id="plan-free">
					<article class="bg-card flex h-full flex-col rounded-2xl p-7 sm:p-8">
						<div class="flex items-center justify-between">
							<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
								Free
							</span>
							<span class="glass-chip inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-foreground/70">
								<Cloud class="size-3" />
								No card
							</span>
						</div>
						<div class="mt-3 flex items-baseline gap-2">
							<span class="text-5xl font-semibold tracking-tight text-foreground">$0</span>
							<span class="text-sm text-muted-foreground">forever</span>
						</div>
						<p class="mt-4 text-sm leading-relaxed text-muted-foreground">
							The full desktop app, plus hosted sharing with sensible caps. No card, no trial clock.
						</p>
						<ul class="mt-6 space-y-3">
							{#each [
								"Everything in the desktop recorder and editor",
								`${free.limits.activeRecasts} active share links, ${free.seats.included} creators`,
								`${gb(free.limits.storageBytes)} hosted storage, ${gb(free.limits.deliveryBytesPerMonth)} delivered a month`,
								"720p playback, 10-minute recordings",
								"Basic watch analytics",
								"Or bring your own bucket and lift every storage cap",
							] as point}
								<li class="flex items-start gap-2.5 text-sm text-foreground/85">
									<Check class="mt-0.5 size-4 shrink-0 text-primary" />
									{point}
								</li>
							{/each}
						</ul>
						<div class="mt-8 pt-2">
							<Button href="/download" size="lg" variant="dark" class="w-full">
								<Download class="size-4" />
								Download free
							</Button>
						</div>
					</article>
				</Reveal>

				<!-- Cloud Pro -->
				<Reveal variant="up" delay={80} id="plan-cloud-pro">
					<article class="glass-card relative flex h-full flex-col overflow-hidden rounded-2xl p-7 ring-1 ring-primary/25 sm:p-8">
						<div
							aria-hidden="true"
							class="pointer-events-none absolute -right-12 -top-12 size-56 rounded-full bg-primary/10 blur-3xl"
						></div>
						<div class="relative flex items-center justify-between">
							<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-primary">
								Pro
							</span>
							<span class="glass-chip inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-foreground/80">
								<Cloud class="size-3 text-primary" />
								Most popular
							</span>
						</div>
						<div class="relative mt-3 flex items-baseline gap-2">
							<span class="text-5xl font-semibold tracking-tight text-foreground">
								{formatUsd(pro)}
							</span>
							<span class="text-sm text-muted-foreground">
								/ month{annual ? ", billed annually" : ""}
							</span>
						</div>
						<p class="relative mt-4 text-sm leading-relaxed text-muted-foreground">
							Covers your first {proPlan.seats.included} creators. Each extra creator is
							{formatUsd(extraSeat)} a month — Loom charges {formatUsd(
								annual ? LOOM.annualMonthlyUsd : LOOM.monthlyUsd,
							)} for every single one.
						</p>
						<div class="relative mt-5 inline-flex items-center gap-2 self-start rounded-full border border-primary/30 bg-primary/8 px-3 py-1 text-[11px] font-medium text-foreground/90">
							<Users class="size-3.5 text-primary" />
							Up to {proPlan.seats.max} creators
						</div>
						<ul class="relative mt-6 space-y-3">
							{#each [
								`${proPlan.limits.activeRecasts} active links, ${gb(proPlan.limits.storageBytes)} storage`,
								`${gb(proPlan.limits.deliveryBytesPerMonth)} delivered a month`,
								"4K playback, 4-hour recordings",
								"Full watch analytics — who watched, how far",
								"Password protection, link expiry, per-viewer access",
								"Custom branding, your own domain, or your own bucket",
							] as point}
								<li class="flex items-start gap-2.5 text-sm text-foreground/85">
									<Check class="mt-0.5 size-4 shrink-0 text-primary" />
									{point}
								</li>
							{/each}
						</ul>

						<div class="relative mt-8 pt-2">
							<form class="flex flex-col gap-2.5" onsubmit={startWithEmail}>
								<label class="sr-only" for="pricing-email">Email address</label>
								<input
									id="pricing-email"
									type="email"
									bind:value={email}
									autocomplete="email"
									placeholder="founder@startup.com"
									class="w-full rounded-lg border border-border-low/70 bg-background/80 px-3.5 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60"
								/>
								<Button type="submit" size="lg" class="group/cta gap-2">
									Start free, upgrade anytime
									<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
								</Button>
							</form>
						</div>
					</article>
				</Reveal>

				<!-- Enterprise -->
				<Reveal variant="right" delay={160}>
					<article class="bg-card flex h-full flex-col rounded-2xl p-7 sm:p-8">
						<div class="flex items-center justify-between">
							<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
								Enterprise
							</span>
							<span class="inline-flex items-center gap-1.5 rounded-full border border-border-low/60 px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-foreground/70">
								<Building2 class="size-3" />
								Talk to us
							</span>
						</div>
						<div class="mt-3 flex items-baseline gap-2">
							<span class="text-5xl font-semibold tracking-tight text-foreground">Custom</span>
						</div>
						<p class="mt-4 text-sm leading-relaxed text-muted-foreground">
							For orgs that need single sign-on, audit trails, and data-residency guarantees.
							Provisioned per agreement, not self-serve.
						</p>
						<ul class="mt-6 space-y-3">
							{#each [
								`Everything in Pro, up to ${PLANS.enterprise.seats.max} creators`,
								"SSO / SAML and SCIM provisioning",
								"Audit log and access controls",
								"Your own S3, R2, Azure or GCP bucket",
								"Data residency control",
								"Dedicated success manager and SLAs",
							] as point}
								<li class="flex items-start gap-2.5 text-sm text-foreground/85">
									<Check class="mt-0.5 size-4 shrink-0 text-primary" />
									{point}
								</li>
							{/each}
						</ul>
						<div class="mt-8 pt-2">
							<Button
								href="mailto:hello@recast.li?subject=Recast%20Enterprise"
								variant="dark"
								size="lg"
								class="w-full gap-2"
							>
								<Mail class="size-4" />
								Contact sales
							</Button>
						</div>
					</article>
				</Reveal>
			</div>
		</Container>
	</Section>

	<!-- The comparison that actually closes: what a team of N pays. -->
	<Section class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Side by side"
				title="What your team actually pays."
				description="Loom bills every creator. We bill the workspace, then {formatUsd(extraSeat)} a head past {proPlan.seats.included}."
			/>

			<Reveal variant="up" class="mt-14">
				<div class="overflow-x-auto rounded-2xl border border-border-low/50">
					<table class="w-full min-w-140 border-collapse text-left">
						<thead>
							<tr class="border-b border-border-low/50 bg-foreground/2 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
								<th scope="col" class="px-5 py-3.5 font-semibold">Team size</th>
								<th scope="col" class="px-5 py-3.5 text-right font-semibold text-primary">Recast Pro</th>
								<th scope="col" class="px-5 py-3.5 text-right font-semibold">Loom Business</th>
								<th scope="col" class="px-5 py-3.5 text-right font-semibold">You save</th>
							</tr>
						</thead>
						<tbody>
							{#each teams as row (row.seats)}
								<tr class="border-b border-border-low/40 last:border-0">
									<th scope="row" class="px-5 py-4 text-sm font-medium text-foreground/85">
										{row.label}
										<span class="ml-1.5 text-xs font-normal text-muted-foreground">
											· {row.seats} {row.seats === 1 ? "person" : "people"}
										</span>
									</th>
									<td class="px-5 py-4 text-right text-sm font-semibold text-foreground">
										{formatUsd(row.recast)}<span class="text-xs font-normal text-muted-foreground">/mo</span>
									</td>
									<td class="px-5 py-4 text-right text-sm text-muted-foreground">
										{formatUsd(row.loom)}<span class="text-xs">/mo</span>
									</td>
									<td class="px-5 py-4 text-right text-sm font-semibold text-primary">
										{row.savingPct}%
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</Reveal>

			<Reveal variant="up" class="mt-6">
				<p class="mx-auto max-w-2xl text-balance text-center text-xs leading-relaxed text-muted-foreground">
					Loom Business is {formatUsd(LOOM.monthlyUsd)} per creator monthly,
					{formatUsd(LOOM.annualMonthlyUsd)} annually (published rates, July 2026). Their filler-word
					and silence removal sits on the {formatUsd(24)} tier — Recast does it free, offline,
					before you make an account.
				</p>
			</Reveal>
		</Container>
	</Section>

	<!-- Full feature matrix -->
	<Section class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Every limit, printed"
				title="What you get, where."
				description="Desktop does the work offline. Cloud adds the sharing surface, with storage you can swap."
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
					Cloud Free needs no card and no trial clock — upgrade to Pro at
					{formatUsd(proPrice(false))} a month whenever you outgrow it.
					Desktop is free forever, no card, no account.
					<a href="mailto:hello@recast.li?subject=Recast%20Enterprise" class="text-foreground underline-offset-2 hover:underline">Talk to us</a> for Enterprise.
				</p>
			</Reveal>
		</Container>
	</Section>

	<Footer />
</main>
