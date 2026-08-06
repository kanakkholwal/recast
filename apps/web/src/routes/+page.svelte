<script lang="ts">
import { goto } from "$app/navigation";
import {
	BeforeAfterSlider,
	Container,
	ExportMock,
	Footer,
	Hero,
	MacWindow,
	PolishGrid,
	RecordMock,
	Reveal,
	Section,
	SectionHeader,
	SeoMeta,
	ShowcasePanel,
} from "$lib/components";
import { prefersReducedMotion } from "$lib/motion-core";
import {
	ArrowRight,
	Check,
	Cloud,
	Compass,
	Download,
	HardDriveUpload,
	KeyRound,
	Minus,
	Play,
	Plus,
	Star,
	X,
} from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import { Image } from "@unpic/svelte";

import { cn } from "@recast/ui/utils";
import { cubicOut } from "svelte/easing";
import { slide } from "svelte/transition";
import {
	beforeAfterClips,
	cloudFeatures,
	contrast,
	editorFeatures,
	extensionBeat,
	faqJsonLd,
	faqs,
	founderUse,
	kindChip,
	openSourceClaims,
	platformDownloads,
	polishFeatures,
	recordingFeatures,
	shareFeatures,
	stabilityChip,
	stabilityChipOnFill,
	storageTiers,
} from "./data";

// Svelte transitions bypass the CSS reduced-motion guard (WAAPI), so gate
// the FAQ expand + waitlist reveal in JS. See motion-core/reduced-motion.
const reduced = $derived(prefersReducedMotion());

// Recast Cloud — the hosted tier. Drive sharing stays the free user-owned
// path; Cloud adds the workspace, analytics, and access controls a raw Drive
// link can't express. Sign-ups are open, so the email field hands off to
// /signup prefilled rather than capturing a waitlist row.
let email = $state("");
const signupHref = $derived(
	email.trim()
		? `/signup?email=${encodeURIComponent(email.trim())}&source=home-cloud`
		: "/signup?source=home-cloud",
);
function startWithEmail(e: SubmitEvent) {
	e.preventDefault();
	goto(signupHref);
}

// Per-feature error flag. Flipped by the <img>'s onerror handler when the
// asset file isn't there yet — the rail card then falls back to its icon
// hero, so a half-produced screenshot batch never shows broken images.
let editorImgErrored = $state<Record<string, boolean>>({});

let openFaq = $state<number | null>(0);

// Drag-to-scroll for the editor rail with flick-to-fling momentum. Pointer
// tracks 1:1 while the button is held; on release we measure the last few
// moves as velocity, then animate a decaying rAF loop until the rail
// either settles or hits a bound. Snap is suspended during the drag (so
// the pointer never fights the snap), then restored so the rail settles
// to the nearest card on release. Keyboard users get the same reach via
// the rail's tabindex (native arrow-key scroll on a focused scroll
// container). Touch users pan natively — the rail only intercepts
// mouse/pen, so a finger flick never conflicts.
//
// Tuned so a quick 200px flick decays over ~700ms (4px initial velocity
// → ~0). Reduced motion keeps the action direct (no inertia).
function dragScroll(node: HTMLElement) {
	const reduced =
		typeof window !== "undefined" &&
		window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

	let down = false;
	let startX = 0;
	let startScroll = 0;
	let snap = "";
	// Velocity sample window: most recent N ms of pointer motion.
	const samples: Array<{ x: number; t: number }> = [];
	const SAMPLE_WINDOW_MS = 120;
	let raf = 0;

	function onDown(e: PointerEvent) {
		if (e.pointerType === "touch") return;
		cancelAnimationFrame(raf);
		down = true;
		startX = e.clientX;
		startScroll = node.scrollLeft;
		snap = node.style.scrollSnapType;
		node.style.scrollSnapType = "none";
		samples.length = 0;
		samples.push({ x: e.clientX, t: performance.now() });
		node.setPointerCapture(e.pointerId);
	}

	function onMove(e: PointerEvent) {
		if (!down) return;
		node.scrollLeft = startScroll - (e.clientX - startX);
		const now = performance.now();
		samples.push({ x: e.clientX, t: now });
		// Drop samples outside the window.
		while (samples.length > 1 && now - samples[0]!.t > SAMPLE_WINDOW_MS) {
			samples.shift();
		}
	}

	function onUp(e: PointerEvent) {
		if (!down) return;
		down = false;
		node.style.scrollSnapType = snap;
		try {
			node.releasePointerCapture(e.pointerId);
		} catch {
			// Some browsers throw if the capture was already released; ignore.
		}
		if (reduced) return;

		// Velocity = Δx / Δt over the recent sample window. Negative because
		// the rail scrolls opposite the pointer (drag right → rail moves
		// leftward in scroll coordinates).
		const first = samples[0];
		const last = samples[samples.length - 1];
		if (!first || !last || first === last) return;
		const dt = last.t - first.t;
		if (dt <= 0) return;
		const vx = (last.x - first.x) / dt; // px/ms
		const scrollV = -vx; // px/ms in scroll direction
		if (Math.abs(scrollV) < 0.1) return; // too slow to bother

		const FRICTION = 0.0014; // per-ms decay coefficient
		let v = scrollV;
		const max = node.scrollWidth - node.clientWidth;
		const tick = () => {
			v *= 1 - FRICTION * 16; // ~60fps frame budget
			node.scrollLeft = clamp(node.scrollLeft + v * 16, 0, max);
			if (Math.abs(v) < 0.05 || node.scrollLeft <= 0 || node.scrollLeft >= max) {
				raf = 0;
				return;
			}
			raf = requestAnimationFrame(tick);
		};
		raf = requestAnimationFrame(tick);
	}

	node.addEventListener("pointerdown", onDown);
	node.addEventListener("pointermove", onMove);
	node.addEventListener("pointerup", onUp);
	node.addEventListener("pointercancel", onUp);
	return {
		destroy() {
			cancelAnimationFrame(raf);
			node.removeEventListener("pointerdown", onDown);
			node.removeEventListener("pointermove", onMove);
			node.removeEventListener("pointerup", onUp);
			node.removeEventListener("pointercancel", onUp);
		},
	};

	function clamp(v: number, lo: number, hi: number) {
		return Math.min(hi, Math.max(lo, v));
	}
}
</script>

<SeoMeta
	title="Record. Polish. Share."
	description="Recast turns a raw screen capture into a polished, shareable demo. Smart auto-edits and a friendly timeline anyone can drive. macOS, Windows, Linux."
	pageTitle="Recast - Record. Polish. Share."
/>

<!-- FAQ rich result. Generated from the same `faqs` array rendered below, so
     the markup never drifts from the on-page copy. -->
<svelte:head>
	{@html `<script type="application/ld+json">${faqJsonLd}<\/script>`}
</svelte:head>

<main class="text-foreground">
	<Hero previewSrc={beforeAfterClips[1].src} />

	<!--
	  Proof section. Permanently dark band regardless of site theme: the
	  `data-theme="dark"` wrapper re-scopes the design tokens, so `bg-canvas`
	  resolves to the dark surface and `text-ink` to the dark foreground. This
	  is the landing anchor for the Hero's "Watch it work" CTA (#proof) so
	  that button is never dead. Static-first: the preview is a plain image
	  today, swappable for a <video> later without touching layout.
	-->
	<div data-theme="dark" id="proof" class="bg-canvas text-ink">
		<Section spacing="tight" class="overflow-hidden">
			<Container>
				<Reveal variant="up">
					<div class="mx-auto flex max-w-3xl flex-col items-center gap-5 text-center">
						<span class="glass-chip inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-ink/80">
							<Play class="size-3.5 text-ink/70" />
							See it work
						</span>
						<h2 class="text-balance text-3xl font-semibold leading-[1.05] tracking-tight sm:text-4xl md:text-5xl">
							Same take.
							<span class="block font-medium italic text-ink/50">
								Polished demo.
							</span>
						</h2>
						<p class="text-pretty max-w-xl text-sm leading-relaxed text-ink-muted sm:text-base">
							One with the OS recorder. One with Recast. Smart zoom, cursor smoothing, padding, and silence cuts already applied by the time you stop.
						</p>
					</div>
				</Reveal>

				<Reveal variant="scale" delay={120}>
					<!--
					  Before/after wipe. One interactive comparison instead of two
					  stacked clips: raw on the left, polished revealed on the right,
					  drag the handle to wipe. Both clips autoplay at the same time
					  and loop independently (they are not frame-synced on purpose:
					  silence-trim changes the polished length, which is the point).
					  The comparison is of the persistent look, so it reads at any
					  handle position; the length delta is shown as proof.
					-->
					<div class="mx-auto mt-12 max-w-5xl">
						<BeforeAfterSlider
							raw={{
								src: beforeAfterClips[0].src,
								label: "Raw",
								durationLabel: beforeAfterClips[0].durationLabel,
							}}
							polished={{
								src: beforeAfterClips[1].src,
								label: "Polished",
								durationLabel: beforeAfterClips[1].durationLabel,
							}}
							applied={beforeAfterClips[1].applied}
						/>
						<p class="mt-4 text-center text-[12.5px] leading-relaxed text-ink-muted">
							Drag to compare. Both clips play at once; the polished cut lands shorter once silence is trimmed.
						</p>
					</div>
				</Reveal>
			</Container>
		</Section>
	</div>

	<!-- Trust strip -->
	<Section spacing="tight" class="border-t border-border-low/60">
		<Container>
			<!--
			  Open-source values strip. Renders first so the page-fold "trust"
			  beat reads as a values statement before it reads as a tech-stack
			  brag. Chips wrap on narrow viewports; divider hairlines disappear
			  when wrapped so we don't get orphan separators.
			-->
			<Reveal variant="blur">
				<!-- Values as a clean editorial row (was pill chips): quieter and
				     more considered, icons muted so nothing competes with the CTA. -->
				<ul class="mx-auto flex max-w-4xl flex-wrap items-center justify-center gap-x-7 gap-y-3">
					{#each openSourceClaims as claim (claim.label)}
						{@const Icon = claim.icon}
						<li class="inline-flex items-center gap-2 text-[13px] font-medium text-foreground/70">
							<Icon class="size-4 text-foreground/40" />
							{claim.label}
						</li>
					{/each}
				</ul>
			</Reveal>

			<Reveal variant="blur" delay={120}>
				<div class="divider-soft mx-auto mt-14 w-24"></div>
				<p class="mt-10 text-center text-[11px] font-semibold uppercase tracking-[0.2em] text-muted-foreground">
					Built on tools makers trust
				</p>
				<div class="mt-9 flex flex-wrap items-center justify-center gap-x-10 gap-y-7 sm:gap-x-14">
					{#each [
						{ name: "Tauri", slug: "tauri", href: "https://tauri.app" },
						{ name: "Rust", slug: "rust", href: "https://www.rust-lang.org" },
						{ name: "Svelte", slug: "svelte", href: "https://svelte.dev" },
						{ name: "TypeScript", slug: "typescript", href: "https://www.typescriptlang.org" },
						{ name: "Vite", slug: "vite", href: "https://vitejs.dev" },
						{ name: "FFmpeg", slug: "ffmpeg", href: "https://ffmpeg.org" },
						{ name: "Tailwind CSS", slug: "tailwindcss", href: "https://tailwindcss.com" },
						{ name: "GitHub", slug: "github", href: "https://github.com/kanakkholwal/recast" },
					] as logo}
						<a
							href={logo.href}
							target="_blank"
							rel="noopener noreferrer"
							class="group flex items-center gap-2 opacity-50 transition-opacity duration-200 hover:opacity-90"
							title={logo.name}
						>
							<Image
								src="https://cdn.simpleicons.org/{logo.slug}/9ca3af"
								alt="{logo.name} logo"
								loading="lazy"
								decoding="async"
								width="20"
								height="20"
								class="h-5 w-5"
							/>
							<span class="text-sm font-semibold tracking-tight text-foreground/55 transition-colors group-hover:text-foreground/85">
								{logo.name}
							</span>
						</a>
					{/each}
				</div>
			</Reveal>
		</Container>
	</Section>

	<!-- Contrast: your OS recorder stops at a file -->
	<Section id="why" class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Why not the built-in recorder"
				title="Your OS recorder stops at a file."
				description="Every laptop ships a screen recorder. None of them ship a demo. The space between a raw capture and something worth sending is the entire job Recast does for you."
				align="center"
			/>

			<div class="glass-card relative mx-auto mt-14 max-w-3xl overflow-hidden rounded-3xl">
				<div class="relative z-10">
					<div class="grid grid-cols-2 border-b border-border-low/50 bg-foreground/2 text-[11px] font-semibold uppercase tracking-[0.16em]">
						<div class="flex items-center gap-2 px-6 py-4 text-muted-foreground">
							<X class="size-3.5" /> Built-in recorder
						</div>
						<div class="flex items-center gap-2 border-l border-border-low/50 px-6 py-4 text-primary">
							<Star class="size-3.5" /> Recast
						</div>
					</div>
					{#each contrast as row, i}
						<Reveal variant={i % 2 === 0 ? "left" : "right"} delay={i * 70}>
							<div class="grid grid-cols-2 {i < contrast.length - 1 ? 'border-b border-border-low/30' : ''}">
								<div class="px-6 py-5 text-sm text-muted-foreground">{row.os}</div>
								<div class="flex items-start gap-2.5 border-l border-border-low/30 bg-foreground/2 px-6 py-5 text-sm text-foreground">
									<Check class="mt-0.5 size-4 shrink-0 text-primary" />
									{row.recast}
								</div>
							</div>
						</Reveal>
					{/each}
				</div>
			</div>
		</Container>
	</Section>

	<!-- Step 1 — Record -->
	<Section id="record" spacing="tight" class="border-t border-border-low/60">
		<Container size="wide">
			<ShowcasePanel tone="neutral">
				<div class="grid items-center gap-16 lg:grid-cols-12 lg:gap-24">
					<!-- Text column: 6/12 so the body copy and feature titles
					     have real horizontal room instead of hugging a narrow
					     rail. The visual (MacWindow) keeps the other half. -->
					<div class="lg:col-span-6">
						<span class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70">
							<span class="size-1.5 rounded-full bg-primary"></span>
							Step 1 · Record
						</span>
						<h2 class="text-balance mt-5 text-3xl font-semibold leading-[1.04] tracking-tight text-foreground sm:text-4xl md:text-5xl lg:text-[3.25rem]">
							Hit record.
							<span class="block font-medium italic text-foreground/45">That's the whole setup.</span>
						</h2>
						<p class="text-pretty mt-6 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Region, window, or full screen. One shortcut starts the capture. No projects, no codecs, no account.
						</p>

						<!-- Recording-side differentiators. Two beats only — both
						     features are usually paywalled in SaaS recorders; here
						     they're in the free local app. -->
						<ul class="mt-12 space-y-6">
							{#each recordingFeatures as f, i}
								{@const Icon = f.icon}
								<Reveal as="li" variant="left" delay={i * 70} class="flex items-start gap-4">
									<span class="glass-chip mt-0.5 grid size-11 shrink-0 place-items-center rounded-xl text-foreground/70">
										<Icon class="size-5" />
									</span>
									<span class="pt-1">
										<span class="block text-[15px] font-semibold tracking-tight text-foreground">{f.title}</span>
										<span class="mt-2 block text-[14px] leading-relaxed text-muted-foreground">{f.description}</span>
									</span>
								</Reveal>
							{/each}
						</ul>

						<div class="mt-12 flex items-center gap-3">
							<Button href="/download" variant="dark" class="gap-2">
								<Download class="size-4" />
								Download free
							</Button>
						</div>
					</div>

					<div class="lg:col-span-6">
						<Reveal variant="morph">
							<MacWindow
								title="Recast"
								class="transition-[transform,box-shadow] duration-300 hover:-translate-y-1 hover:shadow-craft-lg"
							>
								<RecordMock />
							</MacWindow>
						</Reveal>
					</div>
				</div>
			</ShowcasePanel>
		</Container>
	</Section>

	<!-- Step 2 — Auto-polish -->
	<Section id="polish" spacing="tight" class="border-t border-border-low/60">
		<Container size="wide">
			<ShowcasePanel tone="neutral">
				<!-- Header: spans the full panel width so the headline and body
				     can breathe. PolishGrid below gets the full panel width too,
				     so each 4-up tile has room to actually fit its title and
				     description on a single line each. -->
				<div class="mx-auto flex max-w-3xl flex-col items-center text-center">
					<span class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70">
						<span class="size-1.5 rounded-full bg-primary"></span>
						Step 2 · Auto-polish
					</span>
					<h2 class="text-balance mt-5 text-3xl font-semibold leading-[1.04] tracking-tight text-foreground sm:text-4xl md:text-5xl lg:text-[3.25rem]">
						The editing happens
						<span class="block font-medium italic text-foreground/45">while you record.</span>
					</h2>
					<p class="text-pretty mt-5 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
						Smart zoom, cursor smoothing, silence cuts, and padding apply as you record. By the time you stop, the demo is mostly done.
					</p>
				</div>

				<PolishGrid features={polishFeatures} />

				<Reveal variant="up" class="mt-14">
					<figure class="mx-auto max-w-5xl">
						<MacWindow title="Recast · Editor" class="shadow-craft-xl">
							<div class="bg-linear-to-b from-muted/10 to-background p-1.5">
								<Image
									src="/product_preview_hero.webp"
									alt="Recast editor"
									width="1920"
									height="1080"
									loading="lazy"
									decoding="async"
									class="block aspect-video w-full rounded-xl object-cover ring-1 ring-border-low"
								/>
							</div>
						</MacWindow>
						<figcaption class="mt-5 text-center text-[12.5px] leading-relaxed text-muted-foreground">
							The full editor: timeline, zoom regions, annotations, and export presets in one window.
						</figcaption>
					</figure>
				</Reveal>
			</ShowcasePanel>
		</Container>
	</Section>

	<!-- Inside-the-editor tour. Horizontal scroll rail (not a grid) so each
	     feature gets full-width attention; the screenshots/icons are tilted
	     in 3D space so the section reads as a tools showcase, not a spec
	     sheet. Cards extend past the Container's max-width on both edges,
	     fading into the background to suggest "scroll for more". -->
	<Section id="editor" class="overflow-hidden border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="What's in the editor"
				title="Every tool you need. None of the learning curve."
				description="Smart defaults cover most of what a demo needs. When you want to nudge something, the timeline is small, friendly, and deliberately not a real editor. Drag, drop, done."
				align="center"
			/>
		</Container>

		<div class="relative mt-14">
			<!-- Edge fades. Anchored to the viewport so the rail dissolves into
			     the page background instead of ending in a hard cut. -->
			<div
				class="pointer-events-none absolute inset-y-0 left-0 z-20 w-16 bg-linear-to-r from-background to-transparent sm:w-28"
			></div>
			<div
				class="pointer-events-none absolute inset-y-0 right-0 z-20 w-16 bg-linear-to-l from-background to-transparent sm:w-28"
			></div>

			<!-- The rail. `--rail-inset` keeps the first card aligned with the
			     Container gutter on wide viewports while letting later cards
			     flow off-screen. `scrollbar-hide` keeps the chrome clean — the
			     edge fades + drag cursor already telegraph scrollability. -->
			<!-- Focusable + labelled so keyboard/AT users can reach every card
			     (arrow keys scroll a focused scroll container); `dragScroll` makes
			     the grab cursor a real affordance for mouse/pen. -->
			<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
			<!-- tabindex is intentional: a scroll container must be focusable to
			     be keyboard-scrollable; the linter's rule is a known false
			     positive for named scroll regions. -->
			<div
				use:dragScroll
				tabindex="0"
				role="group"
				aria-label="Editor features, scroll horizontally to see all"
				class="editor-rail flex snap-x snap-mandatory gap-5 overflow-x-auto py-10 outline-none ring-primary/50 focus-visible:ring-2 sm:gap-7"
				style="--rail-inset: max(1.25rem, calc((100vw - 80rem) / 2 + 1.25rem)); padding-inline: var(--rail-inset);"
			>
				{#each editorFeatures as feature, i}
					{@const Icon = feature.icon}
					{@const chip = kindChip[feature.kind]}
					<Reveal variant="morph" delay={i * 70} class="snap-center shrink-0">
						<article
							class="group/feat relative flex w-70 flex-col gap-5 sm:w-[320px]"
						>
							<!-- Tilted visual. 3D perspective on the wrapper, the inner
							     plate carries the rotation so hover can soften it. -->
							<div
								class="relative h-52 overflow-hidden rounded-2xl border border-border-low/50 bg-linear-to-br from-foreground/5 via-foreground/2 to-transparent shadow-craft-sm pointer-fine:transition-shadow pointer-fine:duration-200 pointer-fine:ease-out pointer-fine:group-hover/feat:shadow-craft-md"
								style="perspective: 1200px;"
							>
								<!-- Dot grid backdrop. Faint, decorative — the techy vibe. -->
								<div
									aria-hidden="true"
									class="pointer-events-none absolute inset-0 opacity-50"
									style="background-image: radial-gradient(circle, color-mix(in srgb, var(--color-foreground) 8%, transparent) 1px, transparent 1px); background-size: 16px 16px;"
								></div>

								<!-- Corner accents. Tiny CRT-ish brackets to frame the
								     plate without surrounding it in a full border. -->
								<span
									aria-hidden="true"
									class="pointer-events-none absolute left-3 top-3 size-3 border-l border-t border-foreground/30"
								></span>
								<span
									aria-hidden="true"
									class="pointer-events-none absolute right-3 top-3 size-3 border-r border-t border-foreground/30"
								></span>
								<span
									aria-hidden="true"
									class="pointer-events-none absolute bottom-3 left-3 size-3 border-b border-l border-foreground/30"
								></span>
								<span
									aria-hidden="true"
									class="pointer-events-none absolute bottom-3 right-3 size-3 border-b border-r border-foreground/30"
								></span>

								{#if feature.image && !editorImgErrored[feature.title]}
									<!-- Real screenshot in a tilted plate. Hover eases the
									     tilt down so the user can see the image flatter.
									     `onerror` flips the per-card flag so a missing
									     asset falls back to the icon-hero branch below
									     instead of rendering a broken-image glyph. -->
									<div
										class="absolute inset-6 origin-center overflow-hidden rounded-lg border border-border-low/60 shadow-craft-md pointer-fine:transition-transform pointer-fine:duration-200 pointer-fine:ease-out pointer-fine:group-hover/feat:scale-[1.02]"
										style="transform: perspective(900px) rotateX(6deg) rotateY(-10deg); transform-origin: 50% 70%;"
									>
										<Image
											src={feature.image}
											alt={feature.title}
											loading="lazy"
											decoding="async"
											class="block size-full object-cover"
											onerror={() => (editorImgErrored[feature.title] = true)}
										/>
									</div>
								{:else}
									<!-- Icon-as-hero placeholder. The feature's own glyph
									     sits centred and tilted, so a card without a
									     screenshot still carries identity instead of a "no
									     image" hole. -->
									<div
										class="absolute inset-0 grid place-items-center"
										style="transform: perspective(900px) rotateX(8deg) rotateY(-10deg); transform-origin: 50% 70%;"
									>
										<div
											class="relative grid size-28 place-items-center rounded-2xl border border-border-low/60 bg-card/40 shadow-craft-md backdrop-blur-sm"
										>
											<Icon class="size-12 text-foreground/85" />
										</div>
									</div>
								{/if}

								<!-- Mono tag pinned bottom-left, like a chip label on a
								     dev tool. Carries the feature kind for skimmability. -->
								<span
									class={cn(
										"absolute bottom-3 left-1/2 -translate-x-1/2 inline-flex items-center gap-1.5 rounded-full bg-background/70 px-2 py-0.5 font-mono text-[9.5px] font-bold uppercase tracking-[0.14em] ring-1 ring-inset backdrop-blur",
										chip.ring,
									)}
								>
									<span class={cn("size-1.5 rounded-full", chip.dot)}></span>
									{chip.label}
								</span>
							</div>

							<!-- Card content sits below the visual, no enclosing card.
							     Lets the rail feel airier than a tile grid would. -->
							<div class="flex flex-col gap-2 px-1">
								<div class="flex items-center gap-2">
									<span
										class="glass-chip grid size-7 place-items-center rounded-md text-foreground/80 transition-colors group-hover/feat:text-foreground"
									>
										<Icon class="size-3.5" />
									</span>
									<h3 class="text-[15px] font-semibold tracking-tight text-foreground">
										{feature.title}
									</h3>
								</div>
								<p class="text-sm leading-relaxed text-muted-foreground">
									{feature.description}
								</p>
							</div>
						</article>
					</Reveal>
				{/each}
			</div>
		</div>

		<Container>
			<Reveal variant="up" delay={150}>
				<p class="mx-auto mt-6 max-w-3xl text-center text-sm leading-relaxed text-muted-foreground">
					Plus trim &amp; cut, background &amp; padding, drop shadow, custom export presets. Nothing locked behind a "Pro" tier.
				</p>
			</Reveal>
		</Container>
	</Section>

	<!-- Make it yours — extensions as proof of the open, no-lock-in moat.
	     A supporting beat (not a headline) that reinforces "free, offline,
	     yours" rather than pivoting to a generic marketplace pitch. -->
	<Section id="extensions" spacing="tight" class="border-t border-border-low/60">
		<Container size="wide">
			<ShowcasePanel tone="neutral" padding="tight">
				<div class="mx-auto flex max-w-2xl flex-col items-center text-center">
					<span class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70">
						<span class="size-1.5 rounded-full bg-primary"></span>
						Make it yours
					</span>
					<h2 class="text-balance mt-5 text-3xl font-semibold leading-[1.04] tracking-tight text-foreground sm:text-4xl md:text-5xl lg:text-[3.25rem]">
						Open packs.
						<span class="block font-medium italic text-foreground/45">No lock-in.</span>
					</h2>
					<p class="text-pretty mt-5 max-w-lg text-base leading-relaxed text-muted-foreground sm:text-lg">
						Community packs install straight into the editor's pickers. Each one is a manifest and a few static files, hash-checked, with no code and no permissions.
					</p>
				</div>

				<!-- 4-card showcase. Each card is its own panel inside the larger
				     violet showcase, so the grid reads as a sub-gallery of the
				     parent section. -->
				<div class="mt-10 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
					{#each extensionBeat as item, i}
						{@const Icon = item.icon}
						<Reveal variant="up" delay={i * 70} class="h-full">
							<article class="group relative flex h-full flex-col gap-3 rounded-2xl border border-border-low/40 bg-background/85 p-6 shadow-craft-sm transition-[transform,box-shadow,border-color] duration-300 hover:-translate-y-0.5 hover:border-border-low hover:shadow-craft-lg motion-reduce:transition-none">
								<span class="grid size-10 place-items-center rounded-xl bg-foreground/[0.04] text-foreground/75">
									<Icon class="size-5" />
								</span>
								<div>
									<div class="text-[15px] font-semibold tracking-tight text-foreground">{item.title}</div>
									<div class="mt-1.5 text-[14px] leading-relaxed text-muted-foreground">{item.description}</div>
								</div>
							</article>
						</Reveal>
					{/each}
				</div>

				<Reveal variant="up" delay={120} class="mt-10 flex flex-wrap items-center justify-center gap-3">
					<Button href="/extensions" variant="dark" class="gap-2">
						<Compass class="size-4" />
						Explore extensions
					</Button>
					<Button
						href="https://github.com/kanakkholwal/recast/tree/main/extensions"
						variant="light"
						class="gap-2"
						target="_blank"
					>
						<GithubBrand class="size-4" />
						Build a pack
					</Button>
				</Reveal>
			</ShowcasePanel>
		</Container>
	</Section>

	<!-- Step 3 — Share (Google Drive, user-owned) -->
	<Section id="share" spacing="tight" class="border-t border-border-low/60">
		<Container size="wide">
			<ShowcasePanel tone="neutral">
				<div class="grid items-start gap-14 lg:grid-cols-12 lg:gap-20">
					<div class="lg:col-span-5">
						<span class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70">
							<span class="size-1.5 rounded-full bg-primary"></span>
							Step 3 · Share
						</span>
						<h2 class="text-balance mt-5 text-3xl font-semibold leading-[1.04] tracking-tight text-foreground sm:text-4xl md:text-5xl lg:text-[3.25rem]">
							Ship a link.
							<span class="block font-medium italic text-foreground/45">To your Drive.</span>
						</h2>
						<p class="text-pretty mt-5 max-w-md text-base leading-relaxed text-muted-foreground sm:text-lg">
							Connect Drive once. Exports upload straight to your account and hand you a share link. Your video, your storage.
						</p>

						<ul class="mt-10 space-y-5">
							{#each shareFeatures as f, i}
								{@const Icon = f.icon}
								<Reveal as="li" variant="left" delay={i * 70} class="flex items-start gap-4">
									<span class="glass-chip mt-0.5 grid size-11 shrink-0 place-items-center rounded-xl text-foreground/70">
										<Icon class="size-5" />
									</span>
									<span class="pt-1">
										<span class="block text-[15px] font-semibold tracking-tight text-foreground">{f.title}</span>
										<span class="mt-2 block text-[14px] leading-relaxed text-muted-foreground">{f.description}</span>
									</span>
								</Reveal>
							{/each}
						</ul>
					</div>

					<div class="lg:col-span-7">
						<Reveal variant="morph">
							<div class="glass-card relative overflow-hidden rounded-2xl p-7 sm:p-9">
								<div class="relative">
									<span class="glass-chip inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/80">
										<HardDriveUpload class="size-3 text-foreground/70" />
										Google Drive · built in
									</span>

									<h3 class="mt-6 text-2xl font-semibold tracking-tight text-foreground">
										Export → upload → share. In one click.
									</h3>
									<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
										Live upload progress in the success card. The moment it's done, "Copy link" is right there. No second tab, no Recast servers in the middle.
									</p>

									<!-- Mock of the export-success card. Mirrors the real
									     desktop UI (and now loops the upload flow) so the
								     section reads as "this is what you'll actually see",
								     not aspirational marketing. -->
								<div class="mt-7">
									<ExportMock />
								</div>

								<p class="mt-5 inline-flex items-center gap-2 text-xs text-muted-foreground">
									<KeyRound class="size-3.5 text-foreground/70" />
									OAuth scoped to files Recast uploads. Revoke any time from your Google account.
								</p>
							</div>
						</div>
					</Reveal>
				</div>
			</div>
			</ShowcasePanel>
		</Container>
	</Section>

	<!-- Recast Cloud — the hosted offering. The Drive flow above is the free,
	     user-owned default; this is for people who outgrow a raw Drive link. -->
	<Section id="cloud" spacing="tight" class="border-t border-border-low/60">
		<Container size="wide">
			<ShowcasePanel tone="neutral">
				<div class="grid items-start gap-14 lg:grid-cols-12 lg:gap-20">
					<div class="lg:col-span-5">
						<span class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70">
							<span class="size-1.5 rounded-full bg-primary"></span>
							Recast Cloud · live
						</span>
						<h2 class="text-balance mt-5 text-3xl font-semibold leading-[1.04] tracking-tight text-foreground sm:text-4xl md:text-5xl lg:text-[3.25rem]">
							When a Drive link
							<span class="block font-medium italic text-foreground/45">isn't enough.</span>
						</h2>
						<p class="text-pretty mt-6 max-w-md text-base leading-relaxed text-muted-foreground sm:text-lg">
							Loom-style hosted demos, with more of the controls in your hands. Watch analytics, per-viewer access, link expiry.
						</p>

						<!-- Two beats only. The other two (team workspaces, custom player)
						     moved to the pricing page so this section stays marketing-light. -->
						<ul class="mt-10 space-y-5">
							{#each cloudFeatures as f, i}
								{@const Icon = f.icon}
								<Reveal as="li" variant="left" delay={i * 70} class="flex items-start gap-4">
									<span class="glass-chip mt-0.5 grid size-11 shrink-0 place-items-center rounded-xl text-foreground/70">
										<Icon class="size-5" />
									</span>
									<span class="pt-1">
										<span class="block text-[15px] font-semibold tracking-tight text-foreground">{f.title}</span>
										<span class="mt-2 block text-[14px] leading-relaxed text-muted-foreground">{f.description}</span>
									</span>
								</Reveal>
							{/each}
						</ul>
					</div>

					<div class="lg:col-span-7">
						<Reveal variant="morph">
							<div class="glass-card relative overflow-hidden rounded-2xl p-7 sm:p-9">
								<div class="relative">
									<span class="inline-flex items-center gap-2 rounded-full bg-foreground/5 px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.16em] text-foreground/80">
										<Cloud class="size-3.5 text-foreground/70" />
										Recast Cloud
										<span class="text-muted-foreground/40">·</span>
										<span class="size-1.5 rounded-full bg-primary"></span>
										open to everyone
									</span>

									<h3 class="mt-6 text-2xl font-semibold tracking-tight text-foreground">
										Storage-agnostic by design.
									</h3>
									<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
										A sharing + analytics layer that points at whichever storage you want. Yours or ours.
									</p>

									<!-- Storage tier mini-table. Free → BYO storage,
									     Paid → Recast-hosted or your own bucket. -->
									<div class="mt-6 grid grid-cols-1 gap-2.5 sm:grid-cols-2">
										{#each storageTiers as t}
											<div
												class={cn(
													"flex flex-col gap-2 rounded-xl border p-4",
													t.tone === "primary"
														? "border-foreground/20 bg-background/70"
														: "border-border-low/60 bg-background/60",
												)}
											>
												<span class="text-[10px] font-bold uppercase tracking-[0.16em] text-muted-foreground">
													{t.tier}
												</span>
												<span
													class={cn(
														"text-sm font-semibold tracking-tight",
														t.tone === "primary" ? "text-foreground" : "text-foreground/80",
													)}
												>
													{t.label}
												</span>
												<ul class="space-y-1 text-[11.5px] leading-relaxed text-muted-foreground">
													{#each t.lines as line}
														<li class="flex items-start gap-1.5">
															<span class="mt-1.5 size-1 shrink-0 rounded-full bg-foreground/40"></span>
															<span>{line}</span>
														</li>
													{/each}
												</ul>
											</div>
										{/each}
									</div>

									<h4 class="mt-7 text-[13px] font-semibold tracking-tight text-foreground">
										Ship your first hosted demo.
									</h4>
									<p class="mt-1 text-sm leading-relaxed text-muted-foreground">
										Drop your email, upload a take, send the link. Free tier, no card.
									</p>

									<form class="mt-7 flex flex-col gap-2.5 sm:flex-row" onsubmit={startWithEmail}>
										<label class="sr-only" for="home-cloud-email">Work email</label>
										<input
											id="home-cloud-email"
											type="email"
											bind:value={email}
											autocomplete="email"
											placeholder="founder@startup.com"
											class="flex-1 rounded-lg border border-border-low/70 bg-background/80 px-3.5 py-2.5 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground/70 focus:border-primary/60 focus-visible:ring-2 focus-visible:ring-primary/30"
										/>
										<Button type="submit" variant="dark" class="group/cta gap-2">
											Start sharing free
											<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
										</Button>
									</form>

									<p class="mt-4 text-xs text-muted-foreground">
										Already have an account?
										<a href="/login" class="font-semibold text-foreground hover:text-primary">Sign in</a>.
									</p>
								</div>

							</div>
					</Reveal>
				</div>
			</div>
			</ShowcasePanel>
		</Container>
	</Section>

	<!-- Built for solo founders -->
	<Section id="founders" class="border-t border-border-low/60">
		<Container>
			<SectionHeader
				eyebrow="Built for builders"
				title="Shaped for the people who ship."
				description="Opinionated where it matters, out of your way everywhere else."
				align="center"
			/>

			<div class="mt-16 grid grid-cols-1 gap-4 md:grid-cols-3">
				{#each founderUse as item, i}
					{@const Icon = item.icon}
					<Reveal variant="up" delay={i * 70}>
						<article class="flex h-full flex-col rounded-2xl border border-border-low bg-card p-7">
							<span class="grid size-11 place-items-center rounded-xl bg-foreground/[0.04] text-foreground/70">
								<Icon class="size-5" />
							</span>
							<h3 class="mt-6 text-lg font-semibold tracking-tight text-foreground">
								{item.title}
							</h3>
							<p class="mt-2 text-sm leading-relaxed text-muted-foreground">
								{item.description}
							</p>
						</article>
					</Reveal>
				{/each}
			</div>
		</Container>
	</Section>

	<!-- Pricing teaser — the recorder is free, sharing is your storage. -->
	<Section id="pricing-teaser" class="border-t border-border-low/60 ">
		<Container>
			<div class="grid gap-4 md:grid-cols-2">
				<Reveal variant="left">
					<article class="flex h-full flex-col rounded-2xl border border-border-low bg-card p-8">
						<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
							The app
						</span>
						<div class="mt-2 flex items-baseline gap-2">
							<span class="text-4xl font-semibold tracking-tight text-foreground">Free</span>
							<span class="text-sm text-muted-foreground">forever</span>
						</div>
						<p class="mt-3 text-sm leading-relaxed text-muted-foreground">
							Record, auto-polish, edit, and export, all of it offline and without an account. The whole recorder, no asterisk.
						</p>
						<div class="mt-7">
							<Button href="/download" variant="dark" class="gap-2">
								<Download class="size-4" />
								Download
							</Button>
						</div>
					</article>
				</Reveal>

				<Reveal variant="right" delay={80}>
					<!-- Featured tier reads through a stronger border, not a lift: the
					     Free card beside it has no hover, so an animated one broke the pair. -->
					<article class="relative flex h-full flex-col overflow-hidden rounded-2xl border border-foreground/15 bg-card p-8">
						<span class="relative text-[11px] font-semibold uppercase tracking-[0.16em] text-foreground/70">
							Recast Cloud
						</span>
						<div class="relative mt-2 flex items-baseline gap-2">
							<span class="text-4xl font-semibold tracking-tight text-foreground">Hosted</span>
							<span class="text-sm text-muted-foreground">+ controls</span>
						</div>
						<p class="relative mt-3 text-sm leading-relaxed text-muted-foreground">
							A Loom-style hosted layer: watch analytics, per-viewer access, link expiry, team workspaces, and custom branding. Storage stays your call, yours or ours.
						</p>
						<div class="relative mt-7 flex flex-wrap items-center gap-3">
							<Button href="/signup" variant="dark" class="group/cta gap-2">
								Start free
								<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
							</Button>
							<Button href="/pricing" variant="light">
								See pricing
							</Button>
						</div>
					</article>
				</Reveal>
			</div>
		</Container>
	</Section>

	<!-- FAQ. Two-column: sticky title on the left, one-open accordion on the
	     right. Answers only restate claims already made above, so the section
	     never introduces a promise the product doesn't keep. -->
	<Section id="faq" class="border-t border-border-low/60 bg-foreground/1.5 dark:bg-foreground/2">
		<Container>
			<div class="grid gap-12 lg:grid-cols-12 lg:gap-16">
				<div class="lg:col-span-4">
					<div class="lg:sticky lg:top-28">
						<span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
							FAQ
						</span>
						<h2 class="text-balance mt-3 text-3xl font-semibold leading-[1.05] tracking-tight text-foreground sm:text-4xl">
							Frequently asked questions
						</h2>
						<p class="mt-4 text-sm leading-relaxed text-muted-foreground">
							Still wondering something?
							<a
								href="https://github.com/kanakkholwal/recast/discussions"
								target="_blank"
								rel="noopener noreferrer"
								class="font-semibold text-primary hover:underline"
							>
								Ask on GitHub
							</a>.
						</p>
					</div>
				</div>

				<div class="lg:col-span-8">
					<ul class="space-y-3">
						{#each faqs as faq, i (faq.q)}
							{@const open = openFaq === i}
							<li>
								<div
									class={cn(
										"overflow-hidden rounded-2xl border transition-colors",
										open
											? "border-border-low bg-foreground/3"
											: "border-border-low/60 bg-background/50 hover:border-border-low",
									)}
								>
									<button
										type="button"
										onclick={() => (openFaq = open ? null : i)}
										aria-expanded={open}
										aria-controls={`faq-panel-${i}`}
										class="group flex w-full items-start gap-3.5 px-5 py-4 text-left"
									>
										<span aria-hidden="true" class="mt-0.5 shrink-0 text-muted-foreground">
											{#if open}
												<Minus class="size-4" />
											{:else}
												<Plus class="size-4" />
											{/if}
										</span>
										<span class="flex-1 text-[15px] font-semibold tracking-tight text-foreground sm:text-base">
											{faq.q}
										</span>
									</button>
									{#if open}
										<div
											id={`faq-panel-${i}`}
											transition:slide={{ duration: reduced ? 0 : 220, easing: cubicOut }}
											class="overflow-hidden"
										>
											<p class="pb-5 pl-12 pr-5 text-sm leading-relaxed text-muted-foreground">
												{faq.a}
											</p>
										</div>
									{/if}
								</div>
							</li>
						{/each}
					</ul>
				</div>
			</div>
		</Container>
	</Section>

	<!-- Final CTA. Closing bookend. No photo backdrop here — the editorial
	     rule is "no two consecutive photo bands", and the Footer that
	     follows already has one. The CTA carries its presence with a subtle
	     showcase panel + staggered reveals. Each beat lands ~70ms after the
	     previous so the visitor reads chip → headline → body → buttons as
	     one confident breath instead of a single scale-burst. -->
	<Section id="cta" spacing="tight" class="border-t border-border-low/60">
		<Container size="wide">
			<ShowcasePanel tone="neutral" padding="loose">
				<div class="mx-auto flex max-w-3xl flex-col items-center text-center">
					<Reveal variant="scale" duration={420}>
						<div class="glass-chip inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/80">
							<span class="size-1.5 rounded-full bg-primary"></span>
							Ready when you are
						</div>
					</Reveal>

					<Reveal variant="up" delay={70} duration={520}>
						<h2 class="text-balance mt-7 text-4xl font-semibold leading-[1.02] tracking-tight text-foreground sm:text-5xl md:text-6xl lg:text-[4rem]">
							A demo, not a project.
							<span class="block font-medium italic text-foreground/40">Ship it the same day.</span>
						</h2>
					</Reveal>

					<Reveal variant="up" delay={140} duration={520}>
						<p class="text-pretty mt-6 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
							Free forever. No account. Windows is daily-driver stable, macOS and Linux are in active beta.
						</p>
					</Reveal>
				</div>

				<!--
				  Platform downloads as a clean 3-col grid. Stacks on mobile.
				  Windows is the primary, with a small "Recommended" pill
				  above the button; macOS / Linux are equal-weight dark
				  secondaries. Stability chip stays inside the button so
				  the badge never reads as the headline of the row.
				-->
				<div class="mt-10 grid w-full grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 lg:gap-4">
					{#each platformDownloads as p, i}
						{@const Icon = p.icon}
						{@const chip = stabilityChip[p.stability]}
						<Reveal variant="up" delay={210 + i * 70} duration={460} class="h-full">
							<div class="flex h-full flex-col items-stretch gap-2">
								{#if p.stability === "stable"}
									<span class="self-center inline-flex items-center gap-1 rounded-full bg-foreground/8 px-2 py-0.5 text-[9px] font-bold uppercase tracking-[0.14em] text-foreground/80 ring-1 ring-inset ring-foreground/15">
										Recommended
									</span>
								{:else}
									<span class="self-center text-[9px] font-bold uppercase tracking-[0.14em] text-muted-foreground/0">
										.
									</span>
								{/if}
								<Button
									href={p.href}
									size="lg"
									variant={p.variant}
									class="group/dl w-full justify-center gap-2.5"
								>
									<Icon class="size-4" />
									Download for {p.os}
									<span
										class={cn(
											"ml-1 inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-[0.14em] ring-1 ring-inset",
											stabilityChipOnFill,
										)}
									>
										{chip.label}
									</span>
								</Button>
							</div>
						</Reveal>
					{/each}
				</div>

				<!-- Second door out of this section: the desktop app needs no account,
				     but sharing and analytics live behind one, so the CTA row has to
				     offer both instead of only the download. -->
				<Reveal variant="up" delay={420} duration={460} class="mt-9">
					<div class="flex flex-col items-center gap-3">
						<p class="text-sm text-muted-foreground">
							Want hosted sharing, watch analytics, and a team workspace?
						</p>
						<div class="flex flex-wrap items-center justify-center gap-3">
							<Button href="/signup" variant="light" class="group/cta gap-2">
								Share your first demo
								<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
							</Button>
							<Button href="/login" variant="ghost" class="text-muted-foreground">
								Sign in
							</Button>
						</div>
						<a
							href="/download"
							class="group/cta mt-2 inline-flex items-center gap-1.5 text-xs font-semibold text-muted-foreground transition-colors hover:text-foreground"
						>
							All downloads and checksums
							<ArrowRight class="size-3.5 transition-transform group-hover/cta:translate-x-0.5" />
						</a>
					</div>
				</Reveal>
			</ShowcasePanel>
		</Container>
	</Section>

	<Footer />
</main>

<style>
	/* Editor-tour rail: hide the scrollbar (edge fades + drag cursor already
	   telegraph scrollability) and lean on grab/grabbing cursors so the rail
	   reads as draggable on first encounter. */
	.editor-rail {
		scrollbar-width: none;
		-ms-overflow-style: none;
		cursor: grab;
		scroll-behavior: smooth;
	}
	.editor-rail::-webkit-scrollbar {
		display: none;
	}
	.editor-rail:active {
		cursor: grabbing;
	}
</style>
