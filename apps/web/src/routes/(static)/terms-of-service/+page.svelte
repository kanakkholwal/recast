<script lang="ts">
	import {
		Container,
		Eyebrow,
		Footer,
		Reveal,
		Section,
		SeoMeta,
	} from "$lib/components";
	import { ScrollText } from "@recast/icons";

	const lastUpdated = "May 30, 2026";

	type Section = {
		heading: string;
		paragraphs?: string[];
		bullets?: { term?: string; body: string }[];
	};

	const sections: Section[] = [
		{
			heading: "Acceptance",
			paragraphs: [
				"These Terms of Service (the “Terms”) are a contract between you and Recast (“Recast,” “we,” “our”) and govern your use of the Recast desktop application, the website at this domain (the “Site”), and any cloud services we offer (together, the “Service”). By downloading, installing, signing up for, or otherwise using any part of the Service, you agree to these Terms. If you do not agree, do not use the Service.",
			],
		},
		{
			heading: "Who can use Recast",
			paragraphs: [
				"You must be at least 13 years old (16 in the EEA/UK) to use Recast. If you use Recast on behalf of an organization, you represent that you have authority to bind that organization, and “you” refers to that organization.",
			],
		},
		{
			heading: "Your account",
			paragraphs: [
				"You can use the Recast desktop app without an account. Some features — team workspaces, the upcoming Cloud sharing service, and billing — require you to sign up. You are responsible for the accuracy of the information you provide, for keeping your password and authentication tokens confidential, and for everything that happens under your account. Tell us promptly at try-recast@gmail.com if you believe your account has been compromised.",
			],
		},
		{
			heading: "The desktop app",
			paragraphs: [
				"The Recast desktop application is provided under the project’s open-source license as published in the public repository at github.com/kanakkholwal/recast. The source license governs your use of the source code; these Terms govern your use of the hosted Service and any Recast-distributed binaries.",
				"The app runs locally. Recordings, project files, and exports stay on your device unless you explicitly share or upload them. When you opt in to a third-party storage integration — for example, the built-in Google Drive uploader — the file you select is sent directly to your account at that provider. Recast does not store the file on our servers.",
			],
		},
		{
			heading: "Bring-Your-Own-Storage integrations",
			paragraphs: [
				"Recast lets you connect third-party storage providers under your own account. Today that includes Google Drive in the desktop app; additional destinations (for example Cloudinary, autorender.io, Amazon S3, Cloudflare R2, Azure Blob Storage, Google Cloud Storage) are planned for Recast Cloud's free and paid tiers.",
				"When you connect any such provider, the relationship between you and that provider is governed by their terms and privacy policy — not by these Terms. We don't operate that storage account, can't grant or revoke access on your behalf beyond ending the integration, and aren't responsible for their availability, fees, or data-handling practices. You're responsible for keeping your credentials secure, for paying any charges the provider bills you, and for revoking Recast's access (from the provider's UI) if you no longer want it.",
				"OAuth scopes we request are limited to what the integration needs — for Google Drive, that's the drive.file scope, which grants access only to files Recast itself creates in your Drive.",
			],
		},
		{
			heading: "Your content",
			paragraphs: [
				"“Your Content” means anything you record, edit, upload, or share through Recast — including video, audio, captions, cursor data, project files, and metadata.",
				"You keep all rights in Your Content. You grant us a limited, worldwide, non-exclusive, royalty-free license to store, process, transcode, transmit, and display Your Content only as needed to operate the Service for you (for example, to host a shared recording behind a link you created, or to render a thumbnail). This license ends when you delete the content or your account, except for backups that roll off on a normal schedule.",
				"You are responsible for Your Content. You represent that you have the rights and consents needed to capture, share, and store everything you upload — including the consent of any other people visible or audible in your recordings, as required by the laws that apply to you.",
			],
		},
		{
			heading: "Acceptable use",
			paragraphs: [
				"Don’t use Recast to:",
			],
			bullets: [
				{ body: "Record, host, or share content you don’t have the right to capture or distribute, including content that violates someone else’s privacy, publicity, or intellectual property rights." },
				{ body: "Capture audio or video of other people without the consent required by the laws of your jurisdiction." },
				{ body: "Host or distribute malware, phishing pages, sexual content involving minors, threats of violence, or content that promotes self-harm." },
				{ body: "Harass, abuse, or impersonate others, or share another person’s private information without their consent." },
				{ body: "Attempt to break, probe, or overload our infrastructure, reverse engineer the hosted Service, scrape it, or evade rate limits, quotas, or access controls." },
				{ body: "Resell, sublicense, or white-label the Service without our written agreement." },
				{ body: "Use the Service to train machine-learning models on other users’ content." },
			],
			// (closing paragraph below)
		},
		{
			heading: "Enforcement",
			paragraphs: [
				"We may investigate suspected violations and, where we reasonably believe a violation has occurred or is imminent, may suspend or terminate access, remove specific content, or take other proportionate action. We try to give notice and a chance to appeal where we can; we may act without notice when there is an imminent risk of harm or legal exposure.",
			],
		},
		{
			heading: "Paid plans and billing",
			paragraphs: [
				"Paid plans become effective when you start a subscription. Fees, taxes, and billing cycles are shown at checkout. Subscriptions renew automatically at the then-current price for the same term unless you cancel before the renewal date. You can cancel at any time from your account; cancellation takes effect at the end of the current billing period and does not retroactively refund the current period unless required by law.",
				"We may change prices for renewals on at least 30 days’ notice by email. If you don’t agree to the new price, cancel before renewal.",
			],
		},
		{
			heading: "Beta features",
			paragraphs: [
				"We sometimes label features as beta, preview, or experimental. Those features may change, break, or disappear without notice and are provided “as is.” Don’t depend on a beta feature for production-critical workflows.",
			],
		},
		{
			heading: "Third-party services",
			paragraphs: [
				"The Service may integrate with third-party services you choose to connect — for example Google sign-in, Google Drive uploads from the desktop app, payment processors, and (when Recast Cloud is generally available) additional storage providers such as Cloudinary, autorender.io, Amazon S3, Cloudflare R2, Azure Blob Storage, and Google Cloud Storage. Those services are governed by their own terms and privacy policies. We are not responsible for the acts or omissions of third parties, including service outages, billing changes, or content moderation decisions made by the provider.",
			],
		},
		{
			heading: "Intellectual property",
			paragraphs: [
				"The Recast name, logo, brand, marketing site copy, and the hosted Service are owned by us. The desktop application source code is licensed under the terms in the public repository. Nothing in these Terms grants you rights in our trademarks beyond what is necessary to use the Service as intended.",
				"If you give us feedback or suggestions, you grant us a perpetual, irrevocable, royalty-free license to use that feedback without restriction or obligation to you.",
			],
		},
		{
			heading: "Termination",
			paragraphs: [
				"You may stop using Recast at any time and delete your account from settings. We may suspend or terminate your access if you materially breach these Terms, if we are required to by law, or if we discontinue the Service. On termination, your right to use the hosted Service ends; provisions that by their nature should survive (ownership, licenses you grant us in feedback, disclaimers, limitations of liability, indemnity, and dispute resolution) will survive.",
			],
		},
		{
			heading: "Disclaimers",
			paragraphs: [
				"The Service is provided “as is” and “as available.” To the maximum extent permitted by law, we disclaim all warranties, express or implied, including merchantability, fitness for a particular purpose, non-infringement, and any warranty that the Service will be uninterrupted, error-free, or secure. You use the Service at your own risk and are responsible for keeping your own backups of any content that matters to you.",
			],
		},
		{
			heading: "Limitation of liability",
			paragraphs: [
				"To the maximum extent permitted by law, neither party will be liable for indirect, incidental, special, consequential, exemplary, or punitive damages, or for lost profits, revenues, goodwill, or data. Our aggregate liability for any claim arising out of or relating to the Service is limited to the greater of (a) the amount you paid us in the 12 months before the event giving rise to the claim, or (b) one hundred US dollars. Some jurisdictions don’t allow these limits, in which case they apply to the maximum extent permitted.",
			],
		},
		{
			heading: "Indemnity",
			paragraphs: [
				"You agree to defend, indemnify, and hold harmless Recast and its maintainers from claims, damages, liabilities, and reasonable legal fees arising out of or related to Your Content, your use of the Service in violation of these Terms, or your violation of any law or third-party right.",
			],
		},
		{
			heading: "Changes to the Service or Terms",
			paragraphs: [
				"We may change the Service over time — add, modify, or remove features. We may also update these Terms. For material changes to the Terms, we will give reasonable notice by email or an in-app notice before they take effect. Your continued use of the Service after the effective date means you accept the updated Terms; if you don’t agree, stop using the Service.",
			],
		},
		{
			heading: "Governing law and disputes",
			paragraphs: [
				"These Terms are governed by the laws of India, excluding its conflict-of-laws rules. The courts of New Delhi, India have exclusive jurisdiction over any dispute arising out of or relating to these Terms or the Service, subject to any non-waivable consumer-protection rights you have in your country of residence.",
			],
		},
		{
			heading: "Contact",
			paragraphs: [
				"Questions about these Terms? Email try-recast@gmail.com.",
			],
		},
	];
</script>

<SeoMeta
	title="Terms of Service"
	description="The rules that govern using Recast — desktop, website, and Cloud."
	eyebrow="Legal"
/>

<main class="text-foreground">
	<Section spacing="none" class="relative overflow-hidden pt-36 pb-12 md:pt-48 md:pb-16">
		<Container>
			<div class="mx-auto flex max-w-3xl flex-col items-start gap-7 md:items-center md:text-center">
				<Eyebrow icon={ScrollText} variant="primary">Terms of Service</Eyebrow>
				<h1 class="text-balance animate-fade-up text-5xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-6xl md:text-7xl">
					The fine print,
					<span class="mt-2 block font-medium italic text-foreground/40">
						in plain English.
					</span>
				</h1>
				<p class="text-pretty max-w-xl animate-fade-up text-base leading-relaxed text-muted-foreground sm:text-lg" style="animation-delay: 120ms">
					What you can expect from us, what we ask of you, and how we handle the edge cases. By using Recast, you agree to these Terms.
				</p>
				<p class="animate-fade-up text-xs font-semibold uppercase tracking-[0.18em] text-muted-foreground" style="animation-delay: 200ms">
					Last updated · {lastUpdated}
				</p>
			</div>
		</Container>
	</Section>

	<Section spacing="tight" class="border-t border-border-low/60">
		<Container size="narrow">
			<div class="mx-auto max-w-3xl space-y-14">
				{#each sections as section, i}
					<Reveal as="section" delay={i * 40}>
						<h2 class="text-2xl font-semibold tracking-tight text-foreground sm:text-3xl">
							{section.heading}
						</h2>
						{#if section.paragraphs}
							<div class="mt-5 space-y-4 text-pretty text-sm leading-relaxed text-foreground/80 sm:text-base">
								{#each section.paragraphs as p}
									<p>{p}</p>
								{/each}
							</div>
						{/if}
						{#if section.bullets}
							<ul class="mt-5 space-y-3.5 text-pretty text-sm leading-relaxed text-foreground/80 sm:text-base">
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
