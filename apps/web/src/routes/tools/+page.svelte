<script lang="ts">
import { Container, Footer, Section, SeoMeta } from "$lib/components";
import { TOOLS } from "$lib/tools/registry";
import { toolIcon } from "$lib/tools/tool-icons";
import {
	ArrowRight,
	Download,
	Image,
	MousePointerClick,
	ShieldCheck,
	Upload,
	UserX,
	WifiOff,
} from "@recast/icons";
import { Cutout } from "@recast/ui/cutout";
import { LocalIcon } from "@recast/ui/local-icon";
import { prefersReducedMotion } from "$lib/motion-core";
import { fly } from "svelte/transition";
import { cubicOut } from "svelte/easing";

// Same hero entrance pattern as the rest of the public pages: 80ms
// stagger across the eyebrow, headline, body, and CTA. 460ms per
// element lands each in well under a second; the total ladder ends
// around 400ms after first paint.
const reduced = $derived(prefersReducedMotion());
const heroStagger = 80;
const riseM = (delay: number) =>
	reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };

const steps = [
	{
		icon: MousePointerClick,
		title: "Pick a tool",
		body: "Convert, trim, compress, or mute. Each tool is a single focused page.",
	},
	{
		icon: Upload,
		title: "Drop your file",
		body: "It loads into your browser's own video engine. Nothing is uploaded.",
	},
	{
		icon: Download,
		title: "Save the result",
		body: "Download the output instantly. No watermark, no account, no wait.",
	},
];

const privacy = [
	{ icon: WifiOff, label: "Runs offline", body: "Works after the page loads." },
	{ icon: ShieldCheck, label: "No upload", body: "Files never leave your device." },
	{ icon: UserX, label: "No account", body: "No sign-up, no email, no limits." },
];
</script>

<SeoMeta
  title="Free Browser Video Tools"
  description="Convert, trim, compress, and extract from video for free. Everything runs in your browser. Your files are never uploaded."
  eyebrow="Tools"
/>

<main class="flex flex-col pb-8">
  <!-- Hero. Same stagger as the rest of the public pages so the entrance
       reads as one design language across the site. -->
  <Section spacing="none" class="relative overflow-hidden pt-36 pb-20 md:pt-48 md:pb-24">
    <Container size="wide" class="relative">
      <div class="relative z-10 mx-auto flex max-w-2xl flex-col items-center gap-6 text-center">
        <span
          in:fly={riseM(heroStagger * 0)}
          class="inline-flex items-center gap-2 text-body-sm font-medium text-muted-foreground"
        >
          <span class="size-1.5 rounded-full bg-primary"></span>
          Tools
        </span>
        <h1
          in:fly={riseM(heroStagger * 1)}
          class="text-balance text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5rem]"
        >
          Free video tools.
          <span class="block font-medium italic text-muted-foreground">Your files stay on your device.</span>
        </h1>
        <p
          in:fly={riseM(heroStagger * 2)}
          class="text-pretty max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg"
        >
          Quick conversions that run entirely in your browser. Nothing is uploaded, no watermark, no account.
        </p>
        <div in:fly={riseM(heroStagger * 3)} class="mt-1 flex flex-wrap items-center justify-center gap-2">
          <span class="inline-flex items-center gap-1.5 rounded-full bg-paper px-2.5 py-1 text-body-sm font-medium text-muted-foreground">
            <ShieldCheck class="size-3 text-foreground/60" />
            No upload
          </span>
          <span class="inline-flex items-center gap-1.5 rounded-full bg-paper px-2.5 py-1 text-body-sm font-medium text-muted-foreground">
            <UserX class="size-3 text-foreground/60" />
            No account
          </span>
        </div>
      </div>
    </Container>
  </Section>

  <!-- Featured: the screenshot editor is not a worker op, so it sits outside the
       TOOLS registry and gets its own card. -->
  <Container size="wide" class="mb-5">
    <a
      href="/tools/screenshot-editor"
      class="group border-border-low bg-card relative flex flex-col gap-3 overflow-hidden rounded-2xl border p-6 shadow-sm transition-all duration-300 hover:border-border hover:shadow-md sm:flex-row sm:items-center sm:gap-6"
    >
      <Cutout
        corner="tr"
        surface="background"
        radius={14}
        class="flex items-center pt-2 pr-3 pb-4 pl-4"
      >
        <span class="text-primary text-caption font-bold tracking-[0.12em] uppercase">
          Editor
        </span>
      </Cutout>

      <span class="bg-primary/10 text-primary grid size-11 shrink-0 place-items-center rounded-xl">
        <Image class="size-5" />
      </span>

      <div class="min-w-0 flex-1">
        <h3 class="text-base font-semibold tracking-tight">Screenshot Editor</h3>
        <p class="text-muted-foreground mt-1 text-sm leading-relaxed">
          Give a plain screenshot a gradient backdrop, a browser frame, a shadow, and a
          3D tilt, then export at up to 4x.
        </p>
      </div>

      <span
        class="text-primary inline-flex shrink-0 items-center gap-1.5 text-xs font-semibold sm:pr-6"
      >
        Open editor
        <ArrowRight class="size-3.5 transition-transform group-hover:translate-x-0.5" />
      </span>
    </a>
  </Container>

  <!-- Tools grid -->
  <Container size="wide">
    <div class="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
      {#each TOOLS as tool (tool.slug)}
        <a
          href={`/tools/${tool.slug}`}
          class="group relative flex h-full flex-col gap-3 overflow-hidden rounded-2xl border border-border-low bg-card p-6 shadow-sm transition-all duration-300 hover:border-border hover:shadow-md"
        >
          <Cutout
            corner="tr"
            surface="background"
            radius={14}
            class="flex items-center pb-4 pl-4 pr-3 pt-2"
          >
            <span
              class="text-caption font-bold font-medium text-muted-foreground"
            >
              {tool.outputLabel}
            </span>
          </Cutout>

          <span
            class="grid size-11 place-items-center rounded-xl bg-primary/10 text-primary"
          >
            <LocalIcon iconNode={toolIcon(tool.slug)} class="size-5" />
          </span>

          <h3 class="text-base font-semibold tracking-tight">{tool.title}</h3>
          <p class="line-clamp-2 text-sm leading-relaxed text-muted-foreground">
            {tool.tagline}
          </p>

          <span
            class="mt-auto inline-flex items-center gap-1.5 pt-4 text-xs font-semibold text-primary"
          >
            Open tool
            <ArrowRight
              class="size-3.5 transition-transform group-hover:translate-x-0.5"
            />
          </span>
        </a>
      {/each}
    </div>
  </Container>

  <!-- How it works — cutout step cards -->
  <Container size="wide" class="mt-20">
    <div class="mx-auto mb-8 max-w-xl text-center">
      <span
        class="inline-flex items-center gap-2 text-body-sm font-medium text-muted-foreground"
      >
        <span class="size-1.5 rounded-full bg-primary"></span>
        How it works
      </span>
      <h2 class="mt-3 text-balance text-2xl font-semibold tracking-tight sm:text-3xl">
        Three steps, zero uploads
      </h2>
    </div>

    <div class="grid gap-5 sm:grid-cols-3">
      {#each steps as step, i (step.title)}
        <article
          class="group relative flex flex-col gap-3 overflow-hidden rounded-2xl border border-border-low bg-card p-6 shadow-sm transition-all duration-300 hover:shadow-md"
        >
          <Cutout
            corner="tr"
            surface="background"
            radius={14}
            class="flex items-center justify-center pb-4 pl-4 pr-3 pt-2"
          >
            <span class="text-caption font-bold tabular-nums text-muted-foreground">
              0{i + 1}
            </span>
          </Cutout>

          <span
            class="grid size-10 place-items-center rounded-xl bg-primary/10 text-primary"
          >
            <step.icon class="size-5" />
          </span>
          <h3 class="text-base font-semibold tracking-tight">{step.title}</h3>
          <p class="text-sm leading-relaxed text-muted-foreground">{step.body}</p>
        </article>
      {/each}
    </div>
  </Container>

  <!-- Privacy panel — one cutout-tagged surface -->
  <Container size="wide" class="mt-16">
    <div
      class="relative overflow-hidden rounded-2xl border border-border-low bg-card shadow-sm"
    >
      <Cutout corner="tl" surface="background" radius={14} class="pb-3.5 pl-3 pr-4 pt-2.5">
        <span class="text-caption font-semibold text-primary">
          Private by default
        </span>
      </Cutout>

      <div class="grid gap-6 p-6 pt-11 sm:grid-cols-3 sm:p-8 sm:pt-11">
        {#each privacy as item (item.label)}
          <div class="flex items-start gap-3">
            <span
              class="grid size-9 shrink-0 place-items-center rounded-lg bg-paper text-foreground"
            >
              <item.icon class="size-4.5" />
            </span>
            <div>
              <p class="text-sm font-semibold tracking-tight">{item.label}</p>
              <p class="mt-0.5 text-body-sm leading-relaxed text-muted-foreground">
                {item.body}
              </p>
            </div>
          </div>
        {/each}
      </div>
    </div>
  </Container>

  <!-- CTA -->
  <Container size="wide" class="mt-16">
    <div
      class="relative overflow-hidden rounded-2xl border border-border-low bg-card px-6 py-10 text-center shadow-sm sm:px-10"
    >
      <Cutout corner="tr" surface="background" radius={12} class="pb-3 pl-3.5 pr-3.5 pt-1.5">
        <span class="text-caption font-bold tracking-wide text-foreground">Free forever</span>
      </Cutout>

      <div class="mx-auto max-w-xl">
        <h2 class="text-balance text-2xl font-semibold tracking-tight sm:text-3xl">
          Want the full editor?
        </h2>
        <p class="mt-3 text-pretty text-sm leading-relaxed text-muted-foreground sm:text-base">
          These tools are the quick path. Recast for desktop records, polishes,
          and exports a finished demo, offline and in one app.
        </p>
        <a
          href="/download"
          class="mt-6 inline-flex items-center gap-2 rounded-full bg-primary px-5 py-2.5 text-sm font-semibold text-primary-foreground shadow-craft-sm transition-transform"
        >
          Download Recast
          <ArrowRight class="size-4" />
        </a>
      </div>
    </div>
  </Container>

  <Footer />
</main>
