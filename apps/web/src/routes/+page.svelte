<script lang="ts">
import { goto } from "$app/navigation";
import {
	BeforeAfterSlider,
	Container,
	ExportMock,
	FeatureMarquee,
	Footer,
	Hero,
	MacWindow,
	NotchedShelf,
	PillarSection,
	RecordMock,
	Reveal,
	Section,
	SectionLabel,
	SeoMeta,
} from "$lib/components";
import { steps } from "$lib/components/Hero.logic";
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
	Monitor,
	Play,
	Plus,
	Share2,
	Star,
	Video,
	Wand2,
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
	openSourceClaims,
	platformDownloads,
	polishColumns,
	recordColumns,
	shareColumns,
	stabilityChip,
} from "./data";

const reduced = $derived(prefersReducedMotion());

// Same three hues the hero shelf uses, so the closing recap reads as the
// same spine rather than a new set of tags.
const stepGlyph = {
	tangerine: "text-tag-tangerine",
	lavender: "text-tag-lavender",
	green: "text-tag-green",
} as const;

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

let editorImgErrored = $state<Record<string, boolean>>({});

let openFaq = $state<number | null>(0);

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

<svelte:head>
  {@html `<script type="application/ld+json">${faqJsonLd}<\/script>`}
</svelte:head>

<main class="text-foreground">
  <Hero previewSrc={beforeAfterClips[1].src} />

  <div id="proof" class="mx-auto w-full max-w-6xl border-y border-border-low">
    <Section spacing="tight" class="overflow-hidden">
      <Container>
        <Reveal variant="up">
          <div
            class="mx-auto flex max-w-3xl flex-col items-center gap-5 text-center"
          >
            <span
              class="pill inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-body-sm font-medium text-muted-foreground"
            >
              <Play class="size-3.5 text-tag-tangerine" />
              See it work
            </span>
            <h2
              class="text-balance font-medium text-heading sm:text-heading-lg md:text-display"
            >
              Same take.
              <span class="block text-muted-foreground"> Polished demo. </span>
            </h2>
            <p
              class="text-pretty max-w-xl text-sm leading-relaxed text-muted-foreground sm:text-base"
            >
            One with the OS recorder. One with Recast. Same take, polished by the time you stop.
          </p>
          </div>
        </Reveal>

        <Reveal variant="scale" delay={120}>
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
            <p
              class="mt-4 text-center text-caption leading-relaxed text-muted-foreground"
            >
              Drag to compare. Both clips play at once; the polished cut lands
              shorter once silence is trimmed.
            </p>
          </div>
        </Reveal>
      </Container>
    </Section>
  </div>

  <!-- Trust strip -->
  <Section spacing="tight" class="border-t border-border-low">
    <Container>
      <Reveal variant="blur">
        <!-- Values as a clean editorial row (was pill chips): quieter and
				     more considered, icons muted so nothing competes with the CTA. -->
        <ul
          class="mx-auto flex max-w-4xl flex-wrap items-center justify-center gap-x-7 gap-y-3"
        >
          {#each openSourceClaims as claim (claim.label)}
            {@const Icon = claim.icon}
            <li
              class="pill inline-flex w-fit items-center gap-2 px-3 py-1 text-body-sm font-medium text-muted-foreground"
            >
              <Icon class="size-4 text-muted-foreground" />
              {claim.label}
            </li>
          {/each}
        </ul>
      </Reveal>

      <Reveal variant="blur" delay={120}>
        <div class="mx-auto mt-14 h-px w-24 bg-border-low"></div>
        <p
          class="mt-10 text-center text-body-sm font-medium text-muted-foreground"
        >
          Built on tools makers trust
        </p>
        <div
          class="mt-9 flex flex-wrap items-center justify-center gap-x-10 gap-y-7 sm:gap-x-14"
        >
          {#each [{ name: "Tauri", slug: "tauri", href: "https://tauri.app" }, { name: "Rust", slug: "rust", href: "https://www.rust-lang.org" }, { name: "Svelte", slug: "svelte", href: "https://svelte.dev" }, { name: "TypeScript", slug: "typescript", href: "https://www.typescriptlang.org" }, { name: "Vite", slug: "vite", href: "https://vitejs.dev" }, { name: "FFmpeg", slug: "ffmpeg", href: "https://ffmpeg.org" }, { name: "Tailwind CSS", slug: "tailwindcss", href: "https://tailwindcss.com" }, { name: "GitHub", slug: "github", href: "https://github.com/kanakkholwal/recast" }] as logo}
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
              <span
                class="text-sm font-semibold tracking-tight text-muted-foreground transition-colors group-hover:text-foreground"
              >
                {logo.name}
              </span>
            </a>
          {/each}
        </div>
      </Reveal>
    </Container>
  </Section>

  <Section id="why" class="mx-auto max-w-6xl border-t border-border-low">
    <Container>
      <div class="max-w-lg">
        <Reveal variant="up">
          <SectionLabel icon={Monitor} label="Why not the built-in recorder" />
        </Reveal>
        <Reveal variant="up" delay={60} class="mt-5">
          <h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
            Your OS recorder stops at a file
          </h2>
        </Reveal>
        <Reveal variant="up" delay={120} class="mt-4">
          <p class="text-pretty text-body-lg text-muted-foreground">
            Every laptop ships a screen recorder. None of them ship a demo.
          </p>
        </Reveal>
      </div>

      <div class="mt-14 border-y border-border-low">
        <!-- Column heads. The only place --primary appears in this section. -->
        <div class="grid grid-cols-2 border-b border-border-low">
          <div class="flex items-center gap-2 py-4 pr-6 text-body-sm font-medium text-muted-foreground">
            <X class="size-3.5 shrink-0" />
            Built-in recorder
          </div>
          <div class="flex items-center gap-2 border-l border-border-low py-4 pl-6 text-body-sm font-medium text-foreground">
            <Star class="size-3.5 shrink-0 text-primary" />
            Recast
          </div>
        </div>

        {#each contrast as row, i (row.os)}
          <Reveal
            variant="up"
            delay={i * 60}
            class="grid grid-cols-2 {i < contrast.length - 1 ? 'border-b border-border-low' : ''}"
          >
            <div class="py-5 pr-6 text-body-sm text-muted-foreground">
              {row.os}
            </div>
            <div class="flex items-start gap-2.5 border-l border-border-low py-5 pl-6 text-body-sm text-foreground">
              <Check class="mt-0.5 size-4 shrink-0 text-primary" />
              {row.recast}
            </div>
          </Reveal>
        {/each}
      </div>
    </Container>
  </Section>

  <!-- Step 1 — Record -->
  <PillarSection
    id="record"
    index="01"
    icon={Video}
    label="Recast Record"
    accent="tangerine"
    title="It starts with a take"
    description="Region, window, or full screen. One shortcut starts the capture."
    ctaHref="/download"
    ctaLabel="Download free"
    features={recordColumns}
  >
    {#snippet visual()}
      <MacWindow title="Recast" class="mx-auto max-w-4xl">
        <RecordMock />
      </MacWindow>
    {/snippet}
  </PillarSection>

  <!-- Step 2 — Polish -->
  <PillarSection
    id="polish"
    index="02"
    icon={Wand2}
    label="Recast Polish"
    accent="lavender"
    title="The editing happens while you record"
    description="Smart zoom, cursor smoothing and silence cuts apply as you go. The timeline is there when you want it."
    ctaHref="/features"
    ctaLabel="Explore the editor"
    features={polishColumns}
  >
    {#snippet visual()}
      <figure class="mx-auto max-w-5xl">
        <MacWindow title="Recast · Editor">
          <div class="p-1.5">
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
        <figcaption class="mt-5 text-center text-body-sm text-muted-foreground">
          Timeline, zoom regions, annotations and export presets in one window.
        </figcaption>
      </figure>
    {/snippet}
  </PillarSection>

  <!-- Inside the editor. One slow horizontal loop rather than a rail the
       visitor has to drag: the tour is ambient, and hovering parks it. -->
  <Section id="editor" class="mx-auto max-w-6xl border-t border-border-low">
    <Container>
      <div class="max-w-lg">
        <Reveal variant="up">
          <SectionLabel icon={Wand2} label="Inside the editor" accent="lavender" />
        </Reveal>
        <Reveal variant="up" delay={60} class="mt-5">
          <h2 class="font-display text-balance text-heading md:text-heading-lg">
            Every tool you need. None of the learning curve.
          </h2>
        </Reveal>
        <Reveal variant="up" delay={120} class="mt-4">
          <p class="text-pretty text-body-lg text-muted-foreground">
            Smart defaults cover most of it. The timeline is there for the rest.
          </p>
        </Reveal>
      </div>
    </Container>

    <Reveal variant="up" delay={160} class="mt-12">
      <FeatureMarquee items={editorFeatures} />
    </Reveal>

    <Container class="mt-12">
      <p class="text-center text-body-sm text-muted-foreground">
        Plus trim, padding, backgrounds and export presets. Nothing behind a "Pro" tier.
      </p>
    </Container>
  </Section>

<Section id="extensions" class="mx-auto max-w-6xl border-t border-border-low">
    <Container>
      <div class="max-w-lg">
        <Reveal variant="up">
          <SectionLabel icon={Compass} label="Recast Extensions" accent="lavender" />
        </Reveal>
        <Reveal variant="up" delay={60} class="mt-5">
          <h2 class="font-display font-semibold text-balance text-heading md:text-heading-lg">
            Open packs. No lock-in.
          </h2>
        </Reveal>
        <Reveal variant="up" delay={120} class="mt-4">
          <p class="text-pretty text-body-lg text-muted-foreground">
            Community packs install into the editor's pickers: a manifest and static files, hash-checked.
          </p>
        </Reveal>
        <Reveal variant="up" delay={180} class="mt-8 flex flex-wrap gap-3">
          <Button href="/extensions" variant="outline" class="gap-2">
            <Compass class="size-4" />
            Explore extensions
          </Button>
          <Button
            href="https://github.com/kanakkholwal/recast/tree/main/extensions"
            variant="outline"
            class="gap-2"
            target="_blank"
          >
            <GithubBrand class="size-4" />
            Build a pack
          </Button>
        </Reveal>
      </div>
    </Container>

    <!-- Hairline grid, not a row of rounded cards. `gap-px` over a border-coloured
         background draws every separator, however the cells wrap. -->
    <Container class="mt-14">
      <div class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low sm:grid-cols-2 lg:grid-cols-4">
        {#each extensionBeat as item, i (item.title)}
          {@const Icon = item.icon}
          <Reveal variant="up" delay={i * 70} as="article" class="flex h-full flex-col bg-background px-6 py-8">
            <Icon class="size-5 text-tag-lavender [fill-opacity:0.2]" fill="currentColor" />
            <h3 class="mt-4 text-body font-semibold text-foreground">{item.title}</h3>
            <p class="mt-2 text-body-sm text-muted-foreground">{item.description}</p>
          </Reveal>
        {/each}
      </div>
    </Container>
  </Section>

  <!-- Step 3 — Share -->
  <PillarSection
    id="share"
    index="03"
    icon={Share2}
    label="Recast Share"
    accent="green"
    title="Ship a link, not a file"
    description="Connect Drive once and every export lands in your own account, share link in hand."
    ctaHref="/pricing"
    ctaLabel="See sharing"
    features={shareColumns}
  >
    {#snippet visual()}
      <div class="surface-lg mx-auto max-w-3xl p-6 sm:p-8">
        <span
          class="pill inline-flex items-center gap-2 px-3 py-1 text-body-sm font-medium text-muted-foreground"
        >
          <HardDriveUpload
            class="size-3.5 text-tag-green [fill-opacity:0.2]"
            fill="currentColor"
          />
          Google Drive · built in
        </span>
        <h3 class="mt-5 text-heading-sm font-semibold text-foreground">
          Export, upload and share in one click.
        </h3>
        <p class="mt-2 text-body-sm text-muted-foreground">
            Live progress, then "Copy link" is right there. No second tab, no Recast servers in between.
          </p>
        <div class="mt-6">
          <ExportMock />
        </div>
        <p
          class="mt-5 inline-flex items-center gap-2 text-caption text-muted-foreground"
        >
          <KeyRound class="size-3.5" />
          OAuth scoped to files Recast uploads. Revoke any time from your Google
          account.
        </p>
      </div>
    {/snippet}
  </PillarSection>
<Section id="cloud" class="mx-auto max-w-6xl border-t border-border-low">
    <Container>
      <Reveal variant="up">
        <div class="flex items-center gap-4 border-b border-border-low py-5">
          <span class="font-display text-heading-sm leading-none tabular-nums text-border-strong">
            04
          </span>
          <SectionLabel icon={Cloud} label="Recast Cloud" accent="green" />
          <Button href="/pricing" variant="outline" size="sm" class="ml-auto shrink-0">
            See pricing
          </Button>
        </div>
      </Reveal>

      <div class="grid gap-10 py-14 md:grid-cols-12 md:gap-12 md:py-16">
        <div class="md:col-span-5">
          <Reveal variant="up" delay={60}>
            <h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
              When a Drive link isn't enough
            </h2>
          </Reveal>
          <Reveal variant="up" delay={120} class="mt-4">
            <p class="text-pretty text-body-lg text-muted-foreground">
              Hosted demos with the controls in your hands. Storage stays your call.
            </p>
          </Reveal>
        </div>

        <ul class="divide-y divide-border-low md:col-span-6 md:col-start-7">
          {#each cloudFeatures as item, i (item.title)}
            {@const Icon = item.icon}
            <Reveal variant="up" delay={160 + i * 70} as="li" class="flex gap-4 py-5">
              <Icon
                class="mt-0.5 size-5 shrink-0 text-tag-green [fill-opacity:0.2]"
                fill="currentColor"
              />
              <div class="min-w-0">
                <h3 class="text-body font-semibold text-foreground">{item.title}</h3>
                <p class="mt-1 text-body-sm text-muted-foreground">{item.description}</p>
              </div>
            </Reveal>
          {/each}
        </ul>
      </div>
    </Container>

    <!-- One action, on its own band. Nothing competes with it. -->
    <div class="border-y border-border-low bg-paper">
      <Container class="py-14 md:py-16">
        <Reveal variant="up" class="mx-auto max-w-xl text-center">
          <h3 class="font-display text-heading-sm text-foreground">
            Ship your first hosted demo
          </h3>
          <p class="mt-2 text-body-sm text-muted-foreground">
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
              class="flex-1 rounded-lg border border-border-low bg-card px-3.5 py-2.5 text-body-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground focus:border-primary focus-visible:ring-2 focus-visible:ring-primary/30"
            />
            <Button type="submit" variant="dark" class="group/cta gap-2">
              Start sharing free
              <ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
            </Button>
          </form>

          <p class="mt-4 text-caption text-muted-foreground">
            Already have an account?
            <a href="/login" class="font-medium text-foreground hover:text-primary">Sign in</a>.
          </p>
        </Reveal>
      </Container>
    </div>
  </Section>

  <!-- Built for solo founders -->
  <Section id="founders" class="mx-auto max-w-6xl border-t border-border-low">
    <Container>
      <div class="max-w-lg">
        <Reveal variant="up">
          <SectionLabel icon={Star} label="Built for builders" />
        </Reveal>
        <Reveal variant="up" delay={60} class="mt-5">
          <h2 class="font-display text-balance text-heading md:text-heading-lg">
            Shaped for the people who ship
          </h2>
        </Reveal>
        <Reveal variant="up" delay={120} class="mt-4">
          <p class="text-pretty text-body-lg text-muted-foreground">
            Opinionated where it matters, out of your way everywhere else.
          </p>
        </Reveal>
      </div>
    </Container>

    <Container class="mt-14">
      <div class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low md:grid-cols-3">
        {#each founderUse as item, i (item.title)}
          {@const Icon = item.icon}
          <Reveal variant="up" delay={i * 70} as="article" class="flex h-full flex-col bg-background px-6 py-8">
            <Icon class="size-5 text-muted-foreground" />
            <h3 class="mt-4 text-body font-semibold text-foreground">{item.title}</h3>
            <p class="mt-2 text-body-sm text-muted-foreground">{item.description}</p>
          </Reveal>
        {/each}
      </div>
    </Container>
  </Section>

  <!-- Pricing teaser. Two plans as one hairline-split row, not two floating
       cards: they are the same object at two tiers, so a shared rule reads
       truer than two containers sitting side by side. -->
  <Section id="pricing-teaser" class="mx-auto max-w-6xl border-t border-border-low">
    <Container>
      <Reveal variant="up">
        <div class="flex items-center gap-4 border-b border-border-low py-5">
          <span class="font-display text-heading-sm leading-none tabular-nums text-border-strong">
            05
          </span>
          <SectionLabel icon={Star} label="Pricing" />
          <Button href="/pricing" variant="outline" size="sm" class="ml-auto shrink-0">
            Full comparison
          </Button>
        </div>
      </Reveal>

      <div class="grid grid-cols-1 gap-px bg-border-low md:grid-cols-2">
        <Reveal variant="up" delay={60} as="article" class="flex flex-col bg-background py-10 md:pr-10">
          <span class="text-body-sm font-medium text-muted-foreground">The app</span>
          <div class="mt-3 flex items-baseline gap-2">
            <span class="font-display text-display text-foreground">Free</span>
            <span class="text-body-sm text-muted-foreground">forever</span>
          </div>
          <p class="mt-3 max-w-sm text-body-sm text-muted-foreground">
            Record, polish, edit and export. Offline, no account, no asterisk.
          </p>
          <div class="mt-7">
            <Button href="/download" variant="dark" class="gap-2">
              <Download class="size-4" />
              Download
            </Button>
          </div>
        </Reveal>

        <Reveal variant="up" delay={140} as="article" class="flex flex-col bg-background py-10 md:pl-10">
          <span class="text-body-sm font-medium text-muted-foreground">Recast Cloud</span>
          <div class="mt-3 flex items-baseline gap-2">
            <span class="font-display text-display text-foreground">Hosted</span>
            <span class="text-body-sm text-muted-foreground">+ controls</span>
          </div>
          <p class="mt-3 max-w-sm text-body-sm text-muted-foreground">
            Analytics, per-viewer access and link expiry. Storage stays your call.
          </p>
          <div class="mt-7 flex flex-wrap items-center gap-3">
            <Button href="/signup" variant="dark" class="group/cta gap-2">
              Start free
              <ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
            </Button>
            <Button href="/pricing" variant="outline">See pricing</Button>
          </div>
        </Reveal>
      </div>
    </Container>
  </Section>

  <!-- FAQ. Two-column: sticky title on the left, one-open accordion on the
	     right. Answers only restate claims already made above, so the section
	     never introduces a promise the product doesn't keep. -->
  <Section id="faq" class="border-y border-border-low bg-paper">
    <Container>
      <div class="grid gap-12 lg:grid-cols-12 lg:gap-16">
        <div class="lg:col-span-4">
          <div class="lg:sticky lg:top-28">
            <span class="text-body-sm font-medium text-muted-foreground">
              FAQ
            </span>
            <h2 class="text-balance mt-3 text-heading sm:text-heading-lg">
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
                      ? "border-border-low bg-paper"
                      : "border-border-low bg-background/50 hover:border-border-low",
                  )}
                >
                  <button
                    type="button"
                    onclick={() => (openFaq = open ? null : i)}
                    aria-expanded={open}
                    aria-controls={`faq-panel-${i}`}
                    class="group flex w-full items-start gap-3.5 px-5 py-4 text-left"
                  >
                    <span
                      aria-hidden="true"
                      class="mt-0.5 shrink-0 text-muted-foreground"
                    >
                      {#if open}
                        <Minus class="size-4" />
                      {:else}
                        <Plus class="size-4" />
                      {/if}
                    </span>
                    <span
                      class="flex-1 text-body font-semibold text-foreground sm:text-base"
                    >
                      {faq.q}
                    </span>
                  </button>
                  {#if open}
                    <div
                      id={`faq-panel-${i}`}
                      transition:slide={{
                        duration: reduced ? 0 : 220,
                        easing: cubicOut,
                      }}
                      class="overflow-hidden"
                    >
                      <p
                        class="pb-5 pl-12 pr-5 text-sm leading-relaxed text-muted-foreground"
                      >
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

  <!-- Closing CTA. Bookends the hero: the same notched shelf bridges into the
       page's one dark band, and the three-step spine is restated as a compact
       recap so the page ends where it began. -->
  <section id="cta" class="relative bg-background">
    <!-- The shelf stays OUTSIDE the dark subtree: inside it,
         var(--color-background) resolves to the dark canvas and the bridge
         would vanish into the band it is supposed to bridge. -->
    <NotchedShelf fill="text-background" class="h-12 text-body-sm font-medium sm:h-14">
      <Download class="size-4" />
      Download free
    </NotchedShelf>

    <div data-theme="dark" class="-mt-12 bg-canvas pt-12 text-ink sm:-mt-14 sm:pt-14">
      <Container class="pt-16 pb-20 md:pt-20 md:pb-24">
        <div class="mx-auto flex max-w-2xl flex-col items-center text-center">
          <!-- Spine recap. The same three steps, same three hues as the hero
               shelf, reduced to a single line. -->
          <Reveal variant="up" duration={460}>
            <ol class="flex flex-wrap items-center justify-center gap-x-3 gap-y-2 text-body-sm text-ink-muted">
              {#each steps as step, i (step.id)}
                {@const Icon = step.icon}
                <li class="inline-flex items-center gap-2">
                  <Icon
                    class="size-4 shrink-0 [fill-opacity:0.2] {stepGlyph[step.accent]}"
                    fill="currentColor"
                  />
                  {step.label}
                </li>
                {#if i < steps.length - 1}
                  <li aria-hidden="true" class="text-ink/30">→</li>
                {/if}
              {/each}
            </ol>
          </Reveal>

          <Reveal variant="up" delay={70} duration={520} class="mt-7">
            <h2 class="font-display text-balance text-heading-lg text-ink md:text-display">
              A demo, not a project. Ship it the same day.
            </h2>
          </Reveal>

          <Reveal variant="up" delay={140} duration={520} class="mt-5">
            <p class="text-pretty text-body-lg text-ink-muted">
              Free forever, no account needed. Windows is stable; macOS and Linux are in beta.
            </p>
          </Reveal>

          <Reveal variant="up" delay={210} duration={460} class="mt-9">
            <div class="flex flex-wrap items-center justify-center gap-3">
              <Button href="/download" variant="light" size="lg" class="gap-2">
                <Download class="size-4" />
                Download free
              </Button>
              <Button
                href="/signup"
                size="lg"
                class="group/cta gap-2 border-ink/20 bg-transparent text-ink hover:bg-ink/10"
              >
                Share your first demo
                <ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
              </Button>
            </div>
          </Reveal>
        </div>

        <!-- Per-platform row as a hairline grid, not three floating cards: the
             stability label carries the difference, so the cards do not need to. -->
        <Reveal variant="up" delay={280} duration={460} class="mt-14">
          <div class="grid grid-cols-1 gap-px border-y border-ink/15 bg-ink/15 sm:grid-cols-3">
            {#each platformDownloads as p (p.os)}
              {@const Icon = p.icon}
              {@const chip = stabilityChip[p.stability]}
              <a
                href={p.href}
                class="group/dl flex items-center gap-3 bg-canvas px-6 py-6 transition-colors hover:bg-ink/5"
              >
                <Icon class="size-5 shrink-0 text-ink" />
                <span class="flex min-w-0 flex-col text-left">
                  <span class="text-body-sm font-semibold text-ink">Download for {p.os}</span>
                  <span class="text-caption text-ink-muted">{chip.label}</span>
                </span>
                <ArrowRight
                  class="ml-auto size-4 shrink-0 text-ink-muted transition-transform group-hover/dl:translate-x-0.5"
                />
              </a>
            {/each}
          </div>
        </Reveal>

        <Reveal variant="up" delay={350} duration={460} class="mt-8">
          <p class="text-center text-body-sm text-ink-muted">
            <a href="/download" class="font-medium text-ink underline-offset-4 hover:underline">
              All downloads and checksums
            </a>
            ·
            <a href="/login" class="font-medium text-ink underline-offset-4 hover:underline">Sign in</a>
          </p>
        </Reveal>
      </Container>
    </div>
  </section>

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
