<script lang="ts">
import { Container, Eyebrow, Footer, Reveal, Section, SeoMeta } from "$lib/components";
import { ShieldCheck } from "@recast/icons";

const lastUpdated = "May 30, 2026";

type Section = {
	heading: string;
	paragraphs?: string[];
	bullets?: { term?: string; body: string }[];
};

const sections: Section[] = [
	{
		heading: "Summary",
		paragraphs: [
			"Recast is an offline-first desktop screen recorder and editor. Recordings live on your machine. We collect the minimum needed to run the website, your account, and — when you opt in — the upcoming Cloud sharing service.",
			"This policy explains what we collect, why we collect it, how long we keep it, and the choices you have.",
		],
	},
	{
		heading: "Who we are",
		paragraphs: [
			"Recast (“Recast,” “we,” “our”) is an independent product built by Kanak Kholwal. The desktop application is open-source and runs entirely on your device. The website at this domain (the “Site”) and any Recast Cloud service (the “Service”) are operated by us.",
			"For privacy questions or requests, write to try-recast@gmail.com.",
		],
	},
	{
		heading: "What the desktop app collects",
		paragraphs: [
			"The Recast desktop app is local-first. Screen, camera, microphone, and system audio captured during recording stay on your device as .recast project files and exported videos in folders you choose.",
			"The app does not transmit recordings, telemetry, or analytics to us. It only contacts our servers when you explicitly sign in, check for updates, or fetch the public changelog from GitHub.",
			"When you opt in to a third-party storage integration (for example, Google Drive — see below), the app uploads the specific export you choose directly to your account at that provider. The file does not pass through Recast servers.",
		],
	},
	{
		heading: "Google Drive integration (opt-in)",
		paragraphs: [
			"The desktop app can upload finished exports to your own Google Drive. This is strictly opt-in: you connect your Google account once via Google's OAuth 2.0 Device Authorization flow, and from then on the export dialog can ship the file directly to your Drive.",
			"We request a scoped OAuth permission limited to files Recast itself creates in your Drive (Google's drive.file scope). We can read, update, or delete only the files Recast uploads — never your wider Drive. You can revoke this access at any time from your Google Account's connected apps page.",
			"Upload progress, success state, and the resulting Drive link are processed locally in the app. Recast servers do not see the video, the file bytes, or the Drive link unless you separately choose to share it through Recast Cloud once that service is generally available.",
			"We do not retain a copy of any file you upload to Drive. Per-upload history shown inside the app (file name, status, timestamps) lives in your local Recast app state and is cleared when you remove it or uninstall the app.",
		],
	},
	{
		heading: "What the website and account collect",
		bullets: [
			{
				term: "Account details",
				body: "Email address, display name, and a hashed password if you sign up with email. If you sign in with Google, we receive your email, name, profile picture URL, and a stable Google account identifier.",
			},
			{
				term: "Authentication artifacts",
				body: "Session tokens, OAuth refresh tokens (for Desktop sign-in via the Device Authorization flow), password reset tokens, and email verification tokens. These are stored only as long as needed to keep you signed in or to complete a flow.",
			},
			{
				term: "Device session metadata",
				body: 'Each signed-in session also records the IP address and user-agent string of the device that signed in. For Desktop sign-ins the user-agent includes your operating system and computer hostname (e.g. "Recast/0.1.9 (windows; Kanak-PC)") so you can identify the device in your own Devices list and revoke it. This metadata is only shown back to you — never to other users.',
			},
			{
				term: "Waitlist",
				body: "If you join the waitlist, we store your email address and the source page (e.g. /pricing, /waitlist) so we can email you when access opens.",
			},
			{
				term: "Team and workspace data",
				body: "If you create or join a team, we store team name, member roles, invitations, and audit log entries about administrative actions.",
			},
			{
				term: "Billing",
				body: "When paid plans launch, payment is processed by a third-party provider (e.g. Stripe). We receive subscription status, plan, last four digits of the card, and billing country. We do not store full card numbers.",
			},
			{
				term: "Support correspondence",
				body: "If you email us or message us on Discord, we keep that conversation so we can help you and refer back to it.",
			},
			{
				term: "Operational logs",
				body: "Web server access logs (IP address, user agent, request path, timestamp, response status) and error traces. Retained for up to 30 days for security and debugging, then deleted or aggregated.",
			},
		],
	},
	{
		heading: "Recast Cloud (when you opt in)",
		paragraphs: [
			"Recast Cloud is the optional, hosted sharing layer for recordings. It is not included in the free desktop app, and is currently in development.",
			"Cloud is storage-agnostic by design: the recording's video file lives in whichever storage you point Cloud at. On the free Cloud tier you bring your own — for example Google Drive (shipping in the desktop app today), and Cloudinary or autorender.io in the future. On paid plans you can either use Recast-managed storage or point uploads at your own object-storage bucket (Amazon S3, Cloudflare R2, Azure Blob Storage, Google Cloud Storage).",
			"Regardless of where the video bytes live, Recast Cloud stores the share-link metadata (slug, visibility settings, expiry, password salt where applicable), generated thumbnails and captions, and viewer analytics (anonymous view counts, country-level region, duration watched) tied to that recording. When you bring your own storage, we store only a reference to the file in your storage account — we do not copy or rehost the video bytes.",
			"You can delete a Cloud recording at any time. Deletion removes the share-link, analytics, and any Recast-side caches within 30 days. When the video is stored in your own bucket, deletion at our end does not delete the file from your storage — you remain in control there.",
		],
	},
	{
		heading: "Cookies and similar technologies",
		paragraphs: [
			"We use a small number of first-party cookies: a session cookie to keep you signed in, a CSRF token, and a theme preference. We do not use third-party advertising or cross-site tracking cookies.",
			"If we add a privacy-respecting product analytics tool in the future (for example, to count page views on the marketing site), we will update this policy and, where required, ask for your consent.",
		],
	},
	{
		heading: "How we use your information",
		bullets: [
			{
				body: "To create and secure your account, including authenticating sign-ins from the desktop app.",
			},
			{
				body: "To run the Site and any features you actively use (waitlist, team management, Cloud sharing).",
			},
			{
				body: "To send transactional email — verification, password reset, invitations, billing receipts, and security alerts. You cannot opt out of these while you have an active account.",
			},
			{
				body: "To send occasional product updates only if you opted in. You can unsubscribe from any product email at any time.",
			},
			{ body: "To prevent abuse, debug crashes, and protect the integrity of our infrastructure." },
			{ body: "To comply with legal obligations." },
		],
	},
	{
		heading: "Legal bases (EEA/UK users)",
		paragraphs: [
			"We process personal data under one of: performance of a contract (running your account and the Service), our legitimate interests (security, fraud prevention, product improvement), your consent (marketing emails, optional analytics), and legal obligations (tax, accounting, law enforcement requests).",
		],
	},
	{
		heading: "Subprocessors and user-chosen integrations",
		paragraphs: [
			"We rely on a small set of vetted infrastructure providers to run the Site, account system, and (when generally available) Recast Cloud. Separately, you may choose to connect third-party storage providers under your own account — when you do, that provider becomes a recipient of your content under their own terms, not a subprocessor of ours.",
		],
		bullets: [
			{
				term: "Hosting & database (Recast subprocessor)",
				body: "Cloud providers we use to run the Site and store account data.",
			},
			{
				term: "Email delivery (Recast subprocessor)",
				body: "Transactional email provider for verification, reset, and invitations.",
			},
			{
				term: "OAuth identity (user-authorized)",
				body: "Google, only if you choose Google sign-in or Google Drive integration.",
			},
			{
				term: "Recast-managed storage (paid Cloud)",
				body: "Object storage providers we use to host video bytes for paid Cloud plans that use Recast-managed storage, once that tier is generally available.",
			},
			{
				term: "User-chosen storage (Cloud, free + paid BYO)",
				body: "Google Drive (today), Cloudinary, autorender.io (planned for free Cloud), and Amazon S3 / Cloudflare R2 / Azure Blob / Google Cloud Storage (planned for paid Bring-Your-Own-Storage). When you connect any of these, your content is sent directly to your account at that provider; we receive only the metadata needed to keep your share-link in sync.",
			},
			{
				term: "Payments (Recast subprocessor)",
				body: "Stripe (or equivalent), once paid plans are available.",
			},
			{ term: "Error monitoring (Recast subprocessor)", body: "Server-side crash reporting." },
		],
	},
	{
		heading: "Sharing and disclosure",
		paragraphs: [
			"We do not sell personal information and we do not share it for cross-context behavioral advertising. We disclose data only to: (a) the subprocessors above, under contract and only to provide the Service; (b) law enforcement when we receive a valid legal request; and (c) a successor entity in the event of a merger, acquisition, or asset sale, in which case we will notify you and you can delete your account before any transfer.",
		],
	},
	{
		heading: "Data retention",
		bullets: [
			{ term: "Active accounts", body: "Kept until you delete your account." },
			{
				term: "Deleted accounts",
				body: "Permanently removed within 30 days, except for limited records we are required to retain for tax, fraud-prevention, or legal compliance.",
			},
			{
				term: "Waitlist",
				body: "Until you ask to be removed or until 18 months after we stop running a waitlist.",
			},
			{ term: "Server logs", body: "Up to 30 days, then deleted or anonymized." },
			{ term: "Backups", body: "Encrypted backups roll off on a 30-day cycle." },
		],
	},
	{
		heading: "Security",
		paragraphs: [
			"Passwords are stored as salted hashes. Connections to the Site and Cloud are encrypted in transit (TLS). Cloud recordings are encrypted at rest. We limit access to production data to maintainers who need it. No system is perfect; if we ever experience a security incident affecting your data, we will notify you without undue delay.",
		],
	},
	{
		heading: "Your rights",
		paragraphs: [
			"Depending on where you live, you may have the right to access, correct, export, or delete your personal data, to object to or restrict certain processing, and to withdraw consent. You can exercise most of these rights yourself from your account settings, or by emailing try-recast@gmail.com. We respond within 30 days.",
			"California residents have rights under the CCPA/CPRA, including the right to know, delete, correct, and limit. We do not sell or share personal information as defined under those laws.",
		],
	},
	{
		heading: "Children",
		paragraphs: [
			"Recast is not directed to children under 13 (or under 16 in the EEA/UK). We do not knowingly collect personal information from children. If you believe a child has provided us data, email us and we will delete it.",
		],
	},
	{
		heading: "International transfers",
		paragraphs: [
			"We may process and store data in countries other than your own, including the United States. Where required, we use appropriate safeguards — such as Standard Contractual Clauses — for these transfers.",
		],
	},
	{
		heading: "Changes to this policy",
		paragraphs: [
			"We will update this policy when our practices change. The “Last updated” date at the top will reflect the latest revision. For material changes, we will give you reasonable notice by email or an in-app notice before the change takes effect.",
		],
	},
	{
		heading: "Contact",
		paragraphs: ["Questions, requests, or concerns: try-recast@gmail.com."],
	},
];
</script>

<SeoMeta
	title="Privacy Policy"
	description="How Recast collects, uses, and protects your information. Local-first by default; Cloud is opt-in."
	eyebrow="Legal"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-12 md:pt-48 md:pb-16">
		<Container>
			<div class="mx-auto flex max-w-3xl flex-col items-start gap-7 md:items-center md:text-center">
				<Eyebrow icon={ShieldCheck} variant="primary">Privacy Policy</Eyebrow>
				<h1 class="text-balance animate-fade-up text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl">
					Local-first
					<span class="mt-2 block font-medium italic text-muted-foreground">
						by default.
					</span>
				</h1>
				<p class="text-pretty max-w-xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg" style="animation-delay: 120ms">
					Your recordings stay on your device unless you explicitly share them. This page covers what we collect for the website, your account, and the optional Cloud service.
				</p>
				<p class="animate-fade-up text-xs font-semibold font-medium text-muted-foreground" style="animation-delay: 200ms">
					Last updated · {lastUpdated}
				</p>
			</div>
		</Container>
	</Section>

	<Section spacing="tight" class="border-t border-border-low">
		<Container size="narrow">
			<div class="mx-auto max-w-3xl space-y-14">
				{#each sections as section, i}
					<Reveal as="section" delay={i * 40}>
						<h2 class="text-2xl font-semibold tracking-tight text-foreground sm:text-3xl">
							{section.heading}
						</h2>
						{#if section.paragraphs}
							<div class="mt-5 space-y-4 text-pretty text-sm leading-relaxed text-foreground sm:text-base">
								{#each section.paragraphs as p}
									<p>{p}</p>
								{/each}
							</div>
						{/if}
						{#if section.bullets}
							<ul class="mt-5 space-y-3.5 text-pretty text-sm leading-relaxed text-foreground sm:text-base">
								{#each section.bullets as item}
									<li class="flex gap-3">
										<span class="mt-2.5 size-1.5 shrink-0 rounded-full bg-primary/70"></span>
										<span>
											{#if item.term}
												<span class="font-semibold text-foreground">{item.term}.</span>
												<span> {item.body}</span>
											{:else}
												{item.body}
											{/if}
										</span>
									</li>
								{/each}
							</ul>
						{/if}
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<Footer />
</main>
