<script lang="ts">
import { PLANS } from "$lib/billing/catalog";
import {
	Container,
	FaqList,
	Footer,
	Reveal,
	Section,
	SectionLabel,
	SeoMeta,
} from "$lib/components";
import type { IconComponent } from "@recast/icons";
import {
	BarChart3,
	Check,
	Cloud,
	Download,
	Gauge,
	Globe,
	HardDriveUpload,
	KeyRound,
	Link2,
	Lock,
	Mail,
	Server,
	ShieldCheck,
	Tag,
	Timer,
	Users,
	Video,
	Wand2,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import { extraSeatPrice, formatUsd, gb, LOOM, proPrice, teamComparison } from "./pricing.logic";

let annual = $state(false);
const pro = $derived(proPrice(annual));
const extraSeat = $derived(extraSeatPrice(annual));
const teams = $derived(teamComparison(annual));

const free = PLANS.free;
const proPlan = PLANS.pro;
const enterprise = PLANS.enterprise;

type PlanCard = {
	id: string;
	name: string;
	badge?: string;
	price: string;
	unit: string;
	description: string;
	cta: { label: string; href: string; icon: IconComponent; variant: "dark" | "outline" };
	listHeading: string;
	features: Array<{ icon: IconComponent; label: string }>;
	featured?: boolean;
};

const plans = $derived<PlanCard[]>([
	{
		id: "free",
		name: "Free",
		price: "$0",
		unit: "forever",
		description: "The whole desktop app, plus hosted sharing with sensible caps.",
		cta: { label: "Download free", href: "/download", icon: Download, variant: "outline" },
		listHeading: "Key features:",
		features: [
			{ icon: Video, label: "Record, edit and export offline" },
			{ icon: Wand2, label: "Smart zoom, cursor smoothing, silence cuts" },
			{ icon: Link2, label: `${free.limits.activeRecasts} active share links` },
			{ icon: Server, label: `${gb(free.limits.storageBytes)} hosted storage` },
			{ icon: Gauge, label: "720p playback, 10-minute recordings" },
			{ icon: BarChart3, label: "Basic watch analytics" },
		],
	},
	{
		id: "pro",
		name: "Pro",
		badge: "Best value",
		price: formatUsd(pro),
		unit: annual ? "per month, billed annually" : "per month",
		description: `Covers your first ${proPlan.seats.included} creators, then ${formatUsd(extraSeat)} a head.`,
		cta: { label: "Start free", href: "/signup?source=pricing", icon: Cloud, variant: "dark" },
		listHeading: "Everything in Free, plus:",
		features: [
			{ icon: Users, label: `Up to ${proPlan.seats.max} creators` },
			{
				icon: Server,
				label: `${gb(proPlan.limits.storageBytes)} storage, ${gb(proPlan.limits.deliveryBytesPerMonth)} delivered a month`,
			},
			{ icon: Gauge, label: "4K playback, 4-hour recordings" },
			{ icon: BarChart3, label: "Full watch analytics" },
			{ icon: Lock, label: "Password protection and link expiry" },
			{ icon: Globe, label: "Custom branding and your own domain" },
		],
		featured: true,
	},
	{
		id: "enterprise",
		name: "Enterprise",
		price: "Custom",
		unit: "annual billing",
		description: "For orgs that need single sign-on, audit trails and data residency.",
		cta: {
			label: "Contact sales",
			href: "mailto:hello@recast.li?subject=Recast%20Enterprise",
			icon: Mail,
			variant: "outline",
		},
		listHeading: "Everything in Pro, plus:",
		features: [
			{ icon: Users, label: `Up to ${enterprise.seats.max} creators` },
			{ icon: KeyRound, label: "SSO, SAML and SCIM provisioning" },
			{ icon: ShieldCheck, label: "Audit log and access controls" },
			{ icon: HardDriveUpload, label: "Your own S3, R2, Azure or GCP bucket" },
			{ icon: Globe, label: "Data residency control" },
			{ icon: Timer, label: "Dedicated success manager and SLAs" },
		],
	},
]);

const guarantees = [
	{ icon: ShieldCheck, label: "No telemetry" },
	{ icon: HardDriveUpload, label: "Bring your own storage" },
	{ icon: Tag, label: "No card to start" },
];

type Cell = boolean | string;
type Row = { label: string; desktop: Cell; cloudFree: Cell; cloudPro: Cell; enterprise: Cell };
type RowGroup = { heading: string; rows: Row[] };

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
				enterprise: `Up to ${enterprise.seats.max}`,
			},
			{
				label: "Active share links",
				desktop: "Not applicable",
				cloudFree: `${free.limits.activeRecasts}`,
				cloudPro: `${proPlan.limits.activeRecasts}`,
				enterprise: "By agreement",
			},
			{
				label: "Hosted storage",
				desktop: "Not applicable",
				cloudFree: gb(free.limits.storageBytes),
				cloudPro: gb(proPlan.limits.storageBytes),
				enterprise: "By agreement",
			},
			{
				label: "Monthly delivery to viewers",
				desktop: "Not applicable",
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
const columns: { key: ColKey; label: string; featured?: boolean }[] = [
	{ key: "desktop", label: "Desktop" },
	{ key: "cloudFree", label: "Cloud Free" },
	{ key: "cloudPro", label: "Cloud Pro", featured: true },
	{ key: "enterprise", label: "Enterprise" },
];

const faqs = [
	{
		q: "Is the desktop app really free?",
		a: "Yes. Record, polish, edit and export offline with no account and no card. Cloud is the only paid part.",
	},
	{
		q: "Do you charge per creator?",
		a: `Pro covers ${proPlan.seats.included} creators for ${formatUsd(proPrice(false))} a month, then ${formatUsd(proPlan.seats.monthlyUsd)} a head. Loom bills every creator from the first one.`,
	},
	{
		q: "What happens when I hit a Cloud limit?",
		a: "Nothing gets billed behind your back. Links stop being served until you upgrade, retire an old link, or point Recast at your own bucket.",
	},
	{
		q: "Can I use my own storage?",
		a: "Every plan can bring its own bucket. Choosing a named provider (S3, R2, Azure, GCP) and controlling residency are Pro and Enterprise features.",
	},
	{
		q: "Is there a trial, and can I cancel?",
		a: "There is no trial clock. Cloud Free needs no card, you upgrade when you outgrow it, and cancelling stops billing at the end of the period.",
	},
	{
		q: "What happens to my recordings if I stop paying?",
		a: "They stay on your machine, because that is where they were made. Only hosted links stop being served.",
	},
];

const faqJsonLd = JSON.stringify({
	"@context": "https://schema.org",
	"@type": "FAQPage",
	mainEntity: faqs.map((f) => ({
		"@type": "Question",
		name: f.q,
		acceptedAnswer: { "@type": "Answer", text: f.a },
	})),
});
</script>

<SeoMeta
	title="Pricing without the per-seat tax"
	description="Recast Desktop is free forever and runs offline. Recast Cloud Pro is $12/mo for your first three creators, where Loom charges $18 each."
	eyebrow="Pricing"
/>

<svelte:head>
	{@html `<script type="application/ld+json">${faqJsonLd}<\/script>`}
</svelte:head>

<main class="text-foreground">
	<section class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40">
		<Container class="pb-12">
			<Reveal variant="up">
				<SectionLabel icon={Tag} label="Pricing" />
			</Reveal>
			<Reveal variant="up" delay={60} class="mt-5">
				<h1 class="max-w-2xl font-display font-semibold text-balance text-heading-lg md:text-display">
					Plans that don't tax your team
				</h1>
			</Reveal>
			<Reveal variant="up" delay={120} class="mt-4">
				<p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
					The recorder and editor are free forever and run offline. Cloud adds hosted sharing for
					{formatUsd(pro)} a month, covering your first {proPlan.seats.included} creators.
				</p>
			</Reveal>
		</Container>

		<Container class="border-t border-border-low">
			<div class="flex flex-wrap items-center justify-between gap-4 py-4">
				<ul class="flex flex-wrap items-center divide-x divide-border-low">
					{#each guarantees as item (item.label)}
						{@const Icon = item.icon}
						<li
							class="inline-flex items-center gap-2 pr-4 text-body-sm text-muted-foreground not-first:pl-4"
						>
							<Icon class="size-4 shrink-0" />
							{item.label}
						</li>
					{/each}
				</ul>

				<div
					class="inline-flex items-center gap-1 rounded-lg border border-border-low bg-paper p-1"
					role="group"
					aria-label="Billing period"
				>
					<button
						type="button"
						aria-pressed={!annual}
						onclick={() => (annual = false)}
						class={cn(
							"rounded-md px-3 py-1.5 text-body-sm font-medium transition-colors motion-reduce:transition-none",
							annual
								? "text-muted-foreground hover:text-foreground"
								: "bg-background text-foreground shadow-craft-sm",
						)}
					>
						Monthly
					</button>
					<button
						type="button"
						aria-pressed={annual}
						onclick={() => (annual = true)}
						class={cn(
							"inline-flex items-center gap-2 rounded-md px-3 py-1.5 text-body-sm font-medium transition-colors motion-reduce:transition-none",
							annual
								? "bg-background text-foreground shadow-craft-sm"
								: "text-muted-foreground hover:text-foreground",
						)}
					>
						Annual
						<span
							class="rounded-full bg-tag-green/12 px-1.5 py-0.5 text-caption font-medium text-tag-green"
						>
							Save 17%
						</span>
					</button>
				</div>
			</div>
		</Container>
	</section>


	<section class="mx-auto w-full max-w-6xl border-b border-border-low">
		<Container>
			<div class="grid grid-cols-1 gap-px bg-border-low lg:grid-cols-3">
				{#each plans as plan, i (plan.id)}
					{@const CtaIcon = plan.cta.icon}
					<Reveal
						variant="up"
						delay={i * 80}
						as="article"
						class="flex h-full flex-col bg-background"
					>
						<div class={cn("p-6 sm:p-8", plan.featured && "bg-paper")}>
							<div class="flex items-center gap-2">
								<h2 class="font-display text-subheading font-medium text-foreground">
									{plan.name}
								</h2>
								{#if plan.badge}
									<span
										class="rounded-full bg-tag-green/12 px-2 py-0.5 text-caption font-medium text-tag-green"
									>
										{plan.badge}
									</span>
								{/if}
							</div>
							<div class="mt-3 flex items-baseline gap-2">
								<span class="font-display font-semibold text-heading-lg tabular-nums text-foreground">
									{plan.price}
								</span>
								<span class="text-body-sm text-muted-foreground">{plan.unit}</span>
							</div>
							<p class="mt-3 min-h-10 text-pretty text-body-sm text-muted-foreground">
								{plan.description}
							</p>
							<Button
								href={plan.cta.href}
								variant={plan.cta.variant}
								size="lg"
								class="mt-6 w-full gap-2"
							>
								<CtaIcon class="size-4" />
								{plan.cta.label}
							</Button>
						</div>

						<div class="border-t border-border-low p-6 sm:p-8">
							<p class="text-body-sm font-medium text-foreground">{plan.listHeading}</p>
							<ul class="mt-4 space-y-3">
								{#each plan.features as feature (feature.label)}
									{@const Icon = feature.icon}
									<li class="flex items-start gap-2.5 text-body-sm text-muted-foreground">
										<Icon class="mt-0.5 size-4 shrink-0 text-foreground" />
										{feature.label}
									</li>
								{/each}
							</ul>
						</div>
					</Reveal>
				{/each}
			</div>
		</Container>
	</section>

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Users} label="Side by side" accent="green" />
				</div>
			</Reveal>

			<div class="grid gap-10 py-10 md:grid-cols-12 md:gap-12">
				<div class="md:col-span-5">
					<Reveal variant="up" delay={60}>
						<h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
							What your team actually pays
						</h2>
					</Reveal>
					<Reveal variant="up" delay={120} class="mt-4">
						<p class="text-pretty text-body-lg text-muted-foreground">
							Loom bills every creator. We bill the workspace, then {formatUsd(extraSeat)} a head past
							{proPlan.seats.included}.
						</p>
					</Reveal>
				</div>

				<div class="md:col-span-6 md:col-start-7">
					<Reveal variant="up" delay={160}>
						<table class="w-full border-collapse text-left">
							<thead>
								<tr class="border-b border-border-low text-caption font-medium text-muted-foreground">
									<th scope="col" class="py-3 pr-4 font-medium">Team</th>
									<th scope="col" class="px-4 py-3 text-right font-medium text-foreground">
										Recast Pro
									</th>
									<th scope="col" class="px-4 py-3 text-right font-medium">Loom</th>
									<th scope="col" class="py-3 pl-4 text-right font-medium">You save</th>
								</tr>
							</thead>
							<tbody>
								{#each teams as row (row.seats)}
									<tr class="border-b border-border-low">
										<th scope="row" class="py-4 pr-4 text-body-sm font-medium text-foreground">
											{row.label}
											<span class="block text-caption font-normal text-muted-foreground">
												{row.seats} {row.seats === 1 ? "person" : "people"}
											</span>
										</th>
										<td
											class="px-4 py-4 text-right text-body-sm font-medium tabular-nums text-foreground"
										>
											{formatUsd(row.recast)}
										</td>
										<td class="px-4 py-4 text-right text-body-sm tabular-nums text-muted-foreground">
											{formatUsd(row.loom)}
										</td>
										<td
											class="py-4 pl-4 text-right text-body-sm font-medium tabular-nums text-tag-green"
										>
											{row.savingPct}%
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</Reveal>
					<Reveal variant="up" delay={220} class="mt-5">
						<p class="text-caption text-muted-foreground">
							Loom Business is {formatUsd(LOOM.monthlyUsd)} per creator monthly, {formatUsd(
								LOOM.annualMonthlyUsd,
							)} annually (published rates, July 2026). Their filler-word and silence removal sits a
							tier higher again. Recast does it offline, before you make an account.
						</p>
					</Reveal>
				</div>
			</div>
		</Container>
	</Section>

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<Reveal variant="up">
				<div class="flex items-center gap-4 border-b border-border-low pb-5">
					<SectionLabel icon={Cloud} label="Every limit, printed" />
					<Button href="/download" variant="outline" size="sm" class="ml-auto shrink-0">
						Download free
					</Button>
				</div>
			</Reveal>

			<Reveal variant="up" delay={60} class="mt-10">
				<h2 class="max-w-lg  font-medium font-display text-balance text-heading md:text-heading-lg">
					Compare plans
				</h2>
			</Reveal>

			<Reveal variant="up" delay={120} class="mt-8 overflow-x-auto">
				<div class="min-w-190">
					<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr_1fr] border-y border-border-low">
						<div class="py-3 pr-4 text-caption font-medium text-muted-foreground">Feature</div>
						{#each columns as col (col.key)}
							<div
								class={cn(
									"px-4 py-3 text-center text-caption font-medium",
									col.featured ? "text-foreground" : "text-muted-foreground",
								)}
							>
								{col.label}
							</div>
						{/each}
					</div>

					{#each groups as group (group.heading)}
						<div
							class="border-b border-border-low bg-paper px-4 py-2 text-caption font-medium text-muted-foreground"
						>
							{group.heading}
						</div>
						{#each group.rows as row (row.label)}
							<div class="grid grid-cols-[1.6fr_1fr_1fr_1fr_1fr] border-b border-border-low">
								<div class="py-3.5 pr-4 text-body-sm text-foreground">{row.label}</div>
								{#each columns as col (col.key)}
									{@const cell = row[col.key]}
									<div class="flex items-center justify-center px-4 py-3.5 text-center">
										{#if cell === true}
											<Check class="size-4 text-tag-green" />
										{:else if cell === false}
											<span
												aria-label="Not included"
												class="size-1.5 rounded-full bg-border-strong"
											></span>
										{:else}
											<span class="text-caption text-foreground">{cell}</span>
										{/if}
									</div>
								{/each}
							</div>
						{/each}
					{/each}
				</div>
			</Reveal>
		</Container>
	</Section>

	<Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
		<Container>
			<div class="grid gap-10 md:grid-cols-12 md:gap-12">
				<div class="md:col-span-4">
					<Reveal variant="up">
						<h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
							Billing questions
						</h2>
					</Reveal>
					<Reveal variant="up" delay={80} class="mt-4">
						<p class="text-pretty text-body-sm text-muted-foreground">
							Anything else, mail
							<a
								href="mailto:hello@recast.li"
								class="text-foreground underline-offset-4 hover:underline"
							>
								hello@recast.li
							</a>
							and a human answers.
						</p>
					</Reveal>
				</div>
				<Reveal variant="up" delay={120} class="md:col-span-8">
					<FaqList items={faqs} />
				</Reveal>
			</div>
		</Container>
	</Section>

	<Footer />
</main>
