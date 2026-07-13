<script lang="ts">
  import { Container, Footer, SeoMeta } from "$lib/components";
  import { Cutout } from "@recast/ui/cutout";
  import {
    AppWindow,
    ArrowRight,
    Blend,
    Box,
    Clapperboard,
    Download,
    Image,
    MousePointerClick,
    Palette,
    ShieldCheck,
    Sparkles,
    Type,
    UserX,
    WifiOff,
  } from "@lucide/svelte";
  import {
    buildEditorJsonLd,
    EDITOR_DESCRIPTION,
    EDITOR_FAQ,
    EDITOR_TITLE,
    UPSTREAM_URL,
  } from "$lib/tools/screenshot-editor";

  const jsonLd = buildEditorJsonLd();

  const features = [
    {
      icon: Palette,
      title: "Backdrops that do the work",
      body: "Gradients, mesh, patterns, solid colours, your own image, or transparent. Add blur and grain to taste.",
    },
    {
      icon: AppWindow,
      title: "Browser and device frames",
      body: "Drop the shot into a Safari or Chrome window, light or dark, with your own URL. Phone and tablet frames too.",
    },
    {
      icon: Box,
      title: "3D tilt and perspective",
      body: "Rotate and tilt the shot in real 3D space. Presets for a quick angle, sliders when you want it exact.",
    },
    {
      icon: Sparkles,
      title: "Glass and border styles",
      body: "Wrap the shot in a frosted glass card or a clean solid border, with shadows from subtle to dramatic.",
    },
    {
      icon: Type,
      title: "Text and annotations",
      body: "Add headings, arrows, boxes, and circles to point at the thing that matters.",
    },
    {
      icon: Blend,
      title: "Colour adjustments",
      body: "Brightness, contrast, saturation, hue, grayscale, and blur, applied to the shot and not the backdrop.",
    },
    {
      icon: Clapperboard,
      title: "Motion, then MP4",
      body: "Pick a motion preset, stretch the clip on the timeline, and export an MP4 for social.",
    },
    {
      icon: Download,
      title: "Export at up to 4x",
      body: "PNG or JPG at retina resolution, or copy straight to your clipboard and paste it where you need it.",
    },
  ];

  const steps = [
    {
      icon: Image,
      title: "Drop your screenshot",
      body: "Upload it, paste it, or drag it onto the page. It loads straight into your browser.",
    },
    {
      icon: MousePointerClick,
      title: "Make it look good",
      body: "Pick a backdrop, round the corners, add a shadow, tilt it. A template gets you there in one click.",
    },
    {
      icon: Download,
      title: "Export or copy",
      body: "Save a PNG at up to 4x, or copy to the clipboard and paste it into your post.",
    },
  ];

  const privacy = [
    { icon: WifiOff, label: "Runs offline", body: "Works after the page loads." },
    { icon: ShieldCheck, label: "No upload", body: "Your image never leaves your device." },
    { icon: UserX, label: "No account", body: "No sign-up, no email, no watermark." },
  ];
</script>

<SeoMeta title={EDITOR_TITLE} description={EDITOR_DESCRIPTION} eyebrow="Tools" />

<svelte:head>
  {@html `<script type="application/ld+json">${jsonLd}</script>`}
</svelte:head>

<main class="flex flex-col pb-8">
  <!-- Hero -->
  <Container size="wide" class="pt-28 pb-10 sm:pt-32">
    <header class="mx-auto max-w-2xl text-center">
      <span
        class="border-border/50 bg-card text-muted-foreground inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-semibold tracking-[0.16em] uppercase shadow-(--shadow-craft-inset)"
      >
        <ShieldCheck class="text-primary size-3.5" /> No upload · No account
      </span>
      <h1 class="mt-5 text-3xl font-semibold tracking-tight text-balance sm:text-4xl">
        Free screenshot editor
      </h1>
      <p class="text-muted-foreground mt-3 text-base leading-relaxed text-pretty">
        A raw screenshot looks like a bug report. Give it a backdrop, a shadow, and a
        tilt, and it looks like a product. Everything runs in your browser.
      </p>

      <div class="mt-7 flex flex-wrap items-center justify-center gap-3">
        <a
          href="/tools/screenshot-editor/edit"
          class="bg-primary text-primary-foreground shadow-craft-sm inline-flex items-center gap-2 rounded-full px-5 py-2.5 text-sm font-semibold transition-transform hover:-translate-y-0.5"
        >
          Open the editor
          <ArrowRight class="size-4" />
        </a>
        <a
          href="/download"
          class="border-border/60 bg-card hover:border-border inline-flex items-center gap-2 rounded-full border px-5 py-2.5 text-sm font-semibold transition-colors"
        >
          Get the desktop app
        </a>
      </div>
    </header>
  </Container>

  <!-- Before / after. Pure CSS illustration: no asset to load, and it shows the
       exact treatment the editor applies. -->
  <Container size="wide" class="pb-4">
    <div class="grid items-center gap-5 sm:grid-cols-[1fr_auto_1fr]">
      <figure class="flex flex-col gap-3">
        <div class="bg-muted/40 border-border/50 grid place-items-center rounded-2xl border p-6">
          <div class="shot shot-plain" aria-hidden="true">
            <span class="bar w-3/5"></span>
            <span class="bar w-4/5"></span>
            <span class="bar w-2/5"></span>
          </div>
        </div>
        <figcaption
          class="text-muted-foreground text-center text-[11px] font-bold tracking-[0.16em] uppercase"
        >
          Your screenshot
        </figcaption>
      </figure>

      <div class="text-muted-foreground hidden place-items-center sm:grid">
        <ArrowRight class="size-5" />
      </div>

      <figure class="flex flex-col gap-3">
        <div class="stage border-border/50 grid place-items-center rounded-2xl border p-6">
          <div class="tilt" aria-hidden="true">
            <div class="chrome">
              <div class="chrome-bar">
                <i class="dot r"></i><i class="dot y"></i><i class="dot g"></i>
              </div>
              <div class="shot shot-framed">
                <span class="bar w-3/5"></span>
                <span class="bar w-4/5"></span>
                <span class="bar w-2/5"></span>
              </div>
            </div>
          </div>
        </div>
        <figcaption
          class="text-primary text-center text-[11px] font-bold tracking-[0.16em] uppercase"
        >
          Thirty seconds later
        </figcaption>
      </figure>
    </div>
  </Container>

  <!-- Features -->
  <Container size="wide" class="mt-20">
    <div class="mx-auto mb-8 max-w-xl text-center">
      <span
        class="text-muted-foreground inline-flex items-center gap-2 text-[11px] font-semibold tracking-[0.18em] uppercase"
      >
        <span class="bg-primary size-1.5 rounded-full"></span>
        What you get
      </span>
      <h2 class="mt-3 text-2xl font-semibold tracking-tight text-balance sm:text-3xl">
        Everything you need to make a screenshot presentable
      </h2>
    </div>

    <div class="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
      {#each features as feature (feature.title)}
        <article
          class="border-border/50 bg-card flex flex-col gap-3 rounded-2xl border p-6 shadow-sm transition-all duration-300 hover:-translate-y-1 hover:shadow-md"
        >
          <span class="bg-primary/10 text-primary grid size-10 place-items-center rounded-xl">
            <feature.icon class="size-5" />
          </span>
          <h3 class="text-base font-semibold tracking-tight">{feature.title}</h3>
          <p class="text-muted-foreground text-sm leading-relaxed">{feature.body}</p>
        </article>
      {/each}
    </div>
  </Container>

  <!-- How it works -->
  <Container size="wide" class="mt-20">
    <div class="mx-auto mb-8 max-w-xl text-center">
      <span
        class="text-muted-foreground inline-flex items-center gap-2 text-[11px] font-semibold tracking-[0.18em] uppercase"
      >
        <span class="bg-primary size-1.5 rounded-full"></span>
        How it works
      </span>
      <h2 class="mt-3 text-2xl font-semibold tracking-tight text-balance sm:text-3xl">
        Three steps, zero uploads
      </h2>
    </div>

    <div class="grid gap-5 sm:grid-cols-3">
      {#each steps as step, i (step.title)}
        <article
          class="border-border/50 bg-card relative flex flex-col gap-3 overflow-hidden rounded-2xl border p-6 shadow-sm transition-all duration-300 hover:-translate-y-1 hover:shadow-md"
        >
          <Cutout
            corner="tr"
            surface="background"
            radius={14}
            class="flex items-center justify-center pt-2 pr-3 pb-4 pl-4"
          >
            <span class="text-muted-foreground text-[11px] font-bold tabular-nums">
              0{i + 1}
            </span>
          </Cutout>

          <span class="bg-primary/10 text-primary grid size-10 place-items-center rounded-xl">
            <step.icon class="size-5" />
          </span>
          <h3 class="text-base font-semibold tracking-tight">{step.title}</h3>
          <p class="text-muted-foreground text-sm leading-relaxed">{step.body}</p>
        </article>
      {/each}
    </div>
  </Container>

  <!-- Privacy -->
  <Container size="wide" class="mt-16">
    <div class="border-border/50 bg-card relative overflow-hidden rounded-3xl border shadow-sm">
      <Cutout corner="tl" surface="background" radius={14} class="pt-2.5 pr-4 pb-3.5 pl-3">
        <span class="text-primary text-[10px] font-bold tracking-[0.18em] uppercase">
          Private by default
        </span>
      </Cutout>

      <div class="grid gap-6 p-6 pt-11 sm:grid-cols-3 sm:p-8 sm:pt-11">
        {#each privacy as item (item.label)}
          <div class="flex items-start gap-3">
            <span
              class="bg-foreground/5 text-foreground grid size-9 shrink-0 place-items-center rounded-lg"
            >
              <item.icon class="size-4.5" />
            </span>
            <div>
              <p class="text-sm font-semibold tracking-tight">{item.label}</p>
              <p class="text-muted-foreground mt-0.5 text-[13px] leading-relaxed">
                {item.body}
              </p>
            </div>
          </div>
        {/each}
      </div>
    </div>
  </Container>

  <!-- FAQ. Native <details> so the answers are crawlable text and keyboard
       accessible without any JS. -->
  <Container size="wide" class="mt-20">
    <div class="mx-auto max-w-2xl">
      <h2 class="mb-6 text-center text-2xl font-semibold tracking-tight text-balance sm:text-3xl">
        Questions
      </h2>

      <div class="flex flex-col gap-3">
        {#each EDITOR_FAQ as item (item.q)}
          <details
            class="group border-border/50 bg-card rounded-2xl border px-5 py-4 shadow-sm"
          >
            <summary
              class="focus-visible:ring-primary/40 flex cursor-pointer list-none items-center justify-between gap-4 rounded-sm text-sm font-semibold outline-none focus-visible:ring-2"
            >
              {item.q}
              <ArrowRight
                class="text-muted-foreground size-4 shrink-0 transition-transform group-open:rotate-90"
              />
            </summary>
            <p class="text-muted-foreground mt-3 text-sm leading-relaxed">{item.a}</p>
          </details>
        {/each}
      </div>
    </div>
  </Container>

  <!-- CTA -->
  <Container size="wide" class="mt-16">
    <div
      class="border-border/50 bg-card relative overflow-hidden rounded-3xl border px-6 py-10 text-center shadow-sm sm:px-10"
    >
      <Cutout corner="tr" surface="background" radius={12} class="pt-1.5 pr-3.5 pb-3 pl-3.5">
        <span class="text-foreground text-[11px] font-bold tracking-wide">Free forever</span>
      </Cutout>

      <div class="mx-auto max-w-xl">
        <h2 class="text-2xl font-semibold tracking-tight text-balance sm:text-3xl">
          Make your next screenshot look intentional
        </h2>
        <p class="text-muted-foreground mt-3 text-sm leading-relaxed text-pretty sm:text-base">
          No sign-up, no watermark, no upload. Open it and drop an image in.
        </p>
        <a
          href="/tools/screenshot-editor/edit"
          class="bg-primary text-primary-foreground shadow-craft-sm mt-6 inline-flex items-center gap-2 rounded-full px-5 py-2.5 text-sm font-semibold transition-transform hover:-translate-y-0.5"
        >
          Open the editor
          <ArrowRight class="size-4" />
        </a>
      </div>
    </div>
  </Container>

  <!-- Credit. Their work shaped this, so say so where people can see it. -->
  <Container size="wide" class="mt-10">
    <p class="text-muted-foreground text-center text-xs leading-relaxed">
      This editor is a Svelte port of
      <a
        href={UPSTREAM_URL}
        target="_blank"
        rel="noopener noreferrer"
        class="text-foreground font-medium underline-offset-4 hover:underline"
      >
        Screenshot Studio
      </a>
      by Kartik Labhshetwar, used under the Apache 2.0 license. It is a great tool and it
      shaped what this one does.
    </p>
  </Container>

  <Footer />
</main>

<style>
  /* Illustration only. These are literal renderings of the treatment the editor
     applies (gradient backdrop, window chrome, traffic lights), so they are
     fixed colours by design and do not follow the app theme, exactly like the
     editor's own MockupFrame. */
  .shot {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.55rem;
    width: 100%;
    max-width: 20rem;
    aspect-ratio: 16 / 10;
    padding: 1.1rem;
    background: #ffffff;
  }

  .shot .bar {
    height: 0.5rem;
    border-radius: 9999px;
    background: #e4e4e9;
  }

  .shot-plain {
    border: 1px solid #dcdce1;
    border-radius: 2px;
  }

  .stage {
    background: linear-gradient(135deg, #6366f1 0%, #a855f7 50%, #ec4899 100%);
    perspective: 1100px;
  }

  .tilt {
    transform: rotateX(6deg) rotateY(-11deg) rotateZ(1deg) scale(0.94);
    transform-style: preserve-3d;
  }

  .chrome {
    width: 100%;
    max-width: 20rem;
    overflow: hidden;
    border-radius: 10px;
    background: #ffffff;
    box-shadow:
      0 24px 60px rgba(0, 0, 0, 0.35),
      0 8px 20px rgba(0, 0, 0, 0.2);
  }

  .chrome-bar {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    height: 1.6rem;
    padding: 0 0.6rem;
    background: #f2f2f4;
    border-bottom: 1px solid #dcdce1;
  }

  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 9999px;
  }
  .dot.r {
    background: #ff5f57;
  }
  .dot.y {
    background: #febc2e;
  }
  .dot.g {
    background: #28c840;
  }

  .shot-framed {
    aspect-ratio: 16 / 9;
  }

  @media (prefers-reduced-motion: reduce) {
    .tilt {
      transform: none;
    }
  }
</style>
