<script lang="ts">
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
  import {
    ArrowDownToLine,
    CheckCircle2,
    ChevronDown,
    Download,
    Info,
    LifeBuoy,
    ShieldCheck,
    TriangleAlert
  } from "@recast/icons";
  import { AppleBrand, LinuxBrand, WindowsBrand } from "@recast/ui/brand-icons";
  import { Button } from "@recast/ui/button";
  import * as Collapsible from "@recast/ui/collapsible";
  import * as DropdownMenu from "@recast/ui/dropdown-menu";
  import * as Tabs from "@recast/ui/tabs";
  import { cn } from "@recast/ui/utils";
  import { cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import type { PageData } from "./$types";
  import type { OS } from "./data";
  import { installSteps, ISSUES_URL, platforms, ships, stabilityCopy, systemRequirements } from "./data";

  let { data }: { data: PageData } = $props();


  let detectedOS = $state<OS>("Unknown");

  // Hero entrance: same 80ms stagger as the rest of the public pages.
  // 460ms per element lands the whole ladder in well under a second.
  const reduced = $derived(prefersReducedMotion());
  const heroStagger = 80;
  const riseM = (delay: number) =>
    reduced ? { duration: 0 } : { y: 12, duration: 460, delay, easing: cubicOut };

  $effect(() => {
    const ua = window.navigator.userAgent;
    if (ua.includes("Mac")) detectedOS = "macOS";
    else if (ua.includes("Win")) detectedOS = "Windows";
    else if (ua.includes("Linux")) detectedOS = "Linux";
  });

  type Asset = { link: string | null; label: string };

  const platformAssets = $derived<Record<Exclude<OS, "Unknown">, Asset[]>>({
    macOS: [
      { link: data.downloads.macosAppleSilicon, label: "Apple Silicon (.dmg)" },
      { link: data.downloads.macosIntel, label: "Intel (.dmg)" },
    ],
    Windows: [
      { link: data.downloads.windowsExe, label: "Installer (.exe)" },
      { link: data.downloads.windowsMsi, label: "Installer (.msi)" },
    ],
    Linux: [
      { link: data.downloads.linuxAppImage, label: "AppImage (universal)" },
      { link: data.downloads.linuxDeb, label: "Debian / Ubuntu (.deb)" },
      { link: data.downloads.linuxRpm, label: "Red Hat / Fedora (.rpm)" },
    ],
  });

  const primary = $derived(
    detectedOS !== "Unknown" ? platformAssets[detectedOS][0] : null,
  );
  const secondary = $derived(
    detectedOS !== "Unknown" ? platformAssets[detectedOS].slice(1) : [],
  );



 


  let activeTab = $derived(detectedOS !== "Unknown" ? detectedOS : "macOS");

  const detectedIcon = $derived(
    detectedOS === "macOS"
      ? AppleBrand
      : detectedOS === "Windows"
        ? WindowsBrand
        : detectedOS === "Linux"
          ? LinuxBrand
          : Download,
  );


</script>

<SeoMeta
  title="Download Recast"
  description="Download Recast for macOS, Windows, or Linux. Free during beta. The native screen recorder for makers shipping every week."
  eyebrow="Download"
  pageTitle="Download Recast for macOS, Windows, and Linux"
/>

<main class="text-foreground">
  <Section
    spacing="none"
    class="dl-atmosphere relative overflow-hidden pt-36 pb-16 md:pt-48 md:pb-24"
  >
    <HeroBackdrop src="/background-download.webp" tone="strong" />
    <Container class="relative">
      <div
        class="relative z-10 mx-auto flex max-w-3xl flex-col items-center text-center"
      >
        <span
          in:fly={riseM(heroStagger * 0)}
          class="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-foreground/70"
        >
          <Download class="size-3 text-foreground/60" />
          Latest release · {data.version}
        </span>

        <h1
          in:fly={riseM(heroStagger * 1)}
          class="text-balance mt-7 text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5rem]"
        >
          Get Recast for
          <span class="mt-2 block font-medium italic text-foreground/40">
            {detectedOS !== "Unknown" ? detectedOS : "your desktop"}.
          </span>
        </h1>

        <p
          in:fly={riseM(heroStagger * 2)}
          class="text-pretty mt-6 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg"
        >
          Free during beta, no sign-up. The native recorder for makers who'd
          rather ship than open a timeline.
        </p>

        <div class="mt-10 flex flex-col items-center gap-3">
          {#if primary?.link}
            {@const OSIcon = detectedIcon}
            <div
              class="group/dl flex items-stretch overflow-hidden rounded-2xl bg-foreground text-background shadow-craft-sm ring-1 ring-foreground/10 transition-all duration-200 hover:-translate-y-0.5 hover:shadow-craft-floating hover:bg-foreground/90 active:translate-y-0 motion-reduce:transition-none"
            >
              <a
                href={primary.link}
                class="flex items-center gap-3.5 px-5 py-3 transition-colors hover:bg-background/8 sm:gap-4 sm:px-6 sm:py-3.5"
              >
                <OSIcon class="size-6 mx-2" />
                <span class="flex flex-col items-start leading-tight">
                  <span class="text-sm font-semibold sm:text-base">
                    Download for {detectedOS}
                  </span>
                  <span
                    class="mt-0.5 font-mono text-[11px] font-medium opacity-60"
                  >
                    {primary.label}
                  </span>
                </span>
                <ArrowDownToLine
                  class="ml-1 size-4 opacity-70 transition-transform group-hover/dl:translate-y-0.5 sm:ml-2"
                />
              </a>
              {#if secondary.length}
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger
                    class="group/menu grid w-12 shrink-0 place-items-center border-l border-background/15 transition-colors hover:bg-background/8 sm:w-14"
                    aria-label="Other architectures"
                  >
                    <ChevronDown
                      class="size-4 opacity-80 transition-transform duration-200 ease-[cubic-bezier(0.625,0.05,0,1)] group-data-[state=open]/menu:rotate-180"
                    />
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content
                    align="end"
                    sideOffset={10}
                    class="w-72 rounded-xl p-2 shadow-craft-lg"
                  >
                    <DropdownMenu.Label
                      class="px-2.5 pt-1 pb-2 text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground"
                    >
                      Other builds for {detectedOS}
                    </DropdownMenu.Label>
                    {#each secondary as opt}
                      {@const fmt = opt.label.match(/\(([^)]+)\)$/)?.[1] ?? ""}
                      {@const name = opt.label.replace(/\s*\([^)]+\)$/, "")}
                      <DropdownMenu.Item
                        class="group/item flex cursor-pointer items-center justify-between gap-3 rounded-lg px-2 py-2 text-sm font-medium transition-colors duration-200 ease-[cubic-bezier(0.625,0.05,0,1)]"
                        onclick={() =>
                          opt.link && (window.location.href = opt.link)}
                      >
                        <span class="flex items-center gap-2.5">
                          <span
                            class="grid size-8 place-items-center rounded-lg bg-foreground/5 ring-1 ring-foreground/5 transition-colors duration-200 group-hover/item:bg-primary/10 group-hover/item:ring-primary/20"
                          >
                            <OSIcon
                              class="size-4 opacity-70 transition-opacity group-hover/item:opacity-100"
                            />
                          </span>
                          <span class="text-foreground/85">{name}</span>
                        </span>
                        <span
                          class="font-mono text-[10px] font-semibold uppercase tracking-[0.14em] text-muted-foreground/80"
                        >
                          {fmt}
                        </span>
                      </DropdownMenu.Item>
                    {/each}
                    <DropdownMenu.Separator class="my-1.5" />
                    <a
                      href="#all-platforms"
                      class="flex items-center justify-between gap-3 rounded-lg px-2 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-foreground/5 hover:text-foreground"
                    >
                      <span>All platforms & checksums</span>
                      <ArrowDownToLine class="size-3.5 opacity-60" />
                    </a>
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              {/if}
            </div>
          {:else}
            <Button href="#all-platforms" size="lg" class="gap-2">
              View all platforms
              <ArrowDownToLine class="size-4" />
            </Button>
          {/if}

          <a
            href="#all-platforms"
            class="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground transition-colors hover:text-foreground"
          >
            Not on {detectedOS !== "Unknown" ? detectedOS : "this OS"}? See all
            platforms ↓
          </a>

          <!-- macOS users get a one-time Gatekeeper workaround. Surface it
					     up here so they see it BEFORE downloading and don't bounce
					     off the "is damaged" error. Anchors to the full instructions
					     in the macOS tab below. -->
          {#if detectedOS === "macOS"}
            <a
              href="#macos-first-launch"
              class="mt-1 inline-flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-[0.16em] text-amber-600 transition-colors hover:text-amber-500 dark:text-amber-400"
            >
              <TriangleAlert class="size-3" />
              macOS: install with Homebrew, or clear Gatekeeper with one Terminal
              step
            </a>
          {/if}
        </div>
      </div>

      <!-- Honest platform-stability heads-up. Windows is the build I daily-
			     drive; macOS and Linux are early ports that haven't seen the same
			     mileage. Surface it before the user clicks any download so the
			     expectation is set up-front and the GitHub issues link is right
			     there when they hit something. -->
      <Reveal>
        <div
          class="mx-auto mt-16 flex max-w-3xl items-start gap-4 rounded-2xl border border-border-low/40 bg-card/40 p-5 shadow-craft-sm sm:p-6"
        >
          <span
            class="grid size-10 shrink-0 place-items-center rounded-xl bg-foreground/[0.04] text-foreground/70 ring-1 ring-foreground/10"
          >
            <TriangleAlert class="size-4" />
          </span>
          <div class="min-w-0 flex-1">
            <h3 class="text-sm font-semibold tracking-tight text-foreground">
              Heads up: platform stability
            </h3>
            <p class="mt-1.5 text-sm leading-relaxed text-muted-foreground">
              Windows is the daily-driver build.
              <span class="font-semibold text-foreground/85">
                macOS and Linux are early ports
              </span>
              . Don't expect feature parity yet, reach for Windows if you have the
              choice.
            </p>
            <div class="mt-3 flex flex-wrap items-center gap-1.5">
              {#each platforms as p}
                {@const s = stabilityCopy[p.stability]}
                <span
                  class={cn(
                    "inline-flex items-center gap-1.5 rounded-full bg-foreground/4 px-2.5 py-1 font-mono text-[10.5px] font-semibold uppercase tracking-[0.12em] text-foreground/75 ring-1 ring-inset ring-foreground/10",
                  )}
                >
                  <span class={cn("size-1.5 rounded-full", s.dot)}></span>
                  {p.title} · {p.stability === "stable" ? "Stable" : "Beta"}
                </span>
              {/each}
            </div>
            <p class="mt-3 text-xs leading-relaxed text-muted-foreground">
              Hit a bug or papercut? Please file it on
              <a
                href={ISSUES_URL}
                target="_blank"
                rel="noopener noreferrer"
                class="font-semibold text-foreground underline decoration-foreground/30 decoration-1 underline-offset-2 transition-colors hover:text-primary hover:decoration-primary/60"
              >
                GitHub Issues
              </a>
              . I read every one and reply personally.
            </p>
          </div>
        </div>
      </Reveal>

      <!-- Ships with every build -->
      <Reveal>
        <div
          class="mx-auto mt-12 grid max-w-4xl grid-cols-2 gap-px overflow-hidden rounded-2xl border border-border-low/40 bg-border-low/30 sm:grid-cols-4"
        >
          {#each ships as ship}
            {@const Icon = ship.icon}
            <div
              class="flex flex-col gap-2 bg-background/60 p-5 backdrop-blur-md"
            >
              <Icon class="size-4 text-primary" />
              <div>
                <div class="text-sm font-semibold text-foreground">
                  {ship.label}
                </div>
                <div class="mt-0.5 text-xs text-muted-foreground">
                  {ship.value}
                </div>
              </div>
            </div>
          {/each}
        </div>
      </Reveal>
    </Container>
  </Section>

  <!-- System requirements. Surface the honest "works without a GPU" path
	     alongside the recommended hardware so users on entry-level laptops
	     don't bounce thinking they need a discrete GPU. -->
  <Section id="system-requirements" class="border-t border-border-low/60">
    <Container>
      <SectionHeader
        eyebrow="System requirements"
        title="Recording on every machine."
        description="Hardware-accelerated where it counts, with a solid CPU fallback for budget laptops."
      />

      <Reveal>
        <div class="mt-12 grid gap-4 lg:grid-cols-[1fr_2fr]">
          <div class="glass-card flex flex-col gap-3 rounded-2xl p-6">
            <span
              class="glass-chip grid size-10 place-items-center rounded-xl text-foreground/70"
            >
              <Info class="size-4" />
            </span>
            <h3 class="text-base font-semibold tracking-tight">
              How encoding picks itself
            </h3>
            <p class="text-sm leading-relaxed text-muted-foreground">
              Recast tests NVIDIA, AMD, and Intel at startup. If none
              initialise, it falls back to libx264 (CPU) tuned for low-latency
              capture.
            </p>
            <p class="text-xs leading-relaxed text-muted-foreground/80">
              Hardware encoders just let your CPU breathe while you record.
            </p>
          </div>

          <div class="glass-card overflow-hidden rounded-2xl">
            <div
              class="grid grid-cols-[auto_1fr_1fr] items-center gap-x-4 gap-y-0 border-b border-border-low/50 bg-foreground/2 px-5 py-3"
            >
              <span
                class="font-mono text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground"
              >
                Component
              </span>
              <span
                class="font-mono text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground"
              >
                Minimum
              </span>
              <span
                class="font-mono text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground"
              >
                Recommended
              </span>
            </div>
            <ul>
              {#each systemRequirements as req}
                {@const Icon = req.icon}
                <li
                  class="grid grid-cols-[auto_1fr_1fr] items-start gap-x-4 gap-y-1 border-b border-border-low/40 px-5 py-4 last:border-b-0"
                >
                  <span class="flex items-center gap-2.5 pt-0.5">
                    <span
                      class="grid size-8 place-items-center rounded-lg bg-foreground/5 text-foreground/70 ring-1 ring-foreground/5"
                    >
                      <Icon class="size-4" />
                    </span>
                    <span
                      class="text-sm font-semibold tracking-tight text-foreground"
                    >
                      {req.label}
                    </span>
                  </span>
                  <span class="text-sm leading-relaxed text-muted-foreground">
                    {req.minimum}
                  </span>
                  <span class="text-sm leading-relaxed text-foreground/85">
                    {req.recommended}
                  </span>
                </li>
              {/each}
            </ul>
          </div>
        </div>
      </Reveal>
    </Container>
  </Section>

  <Section id="all-platforms" class="border-t border-border-low/60">
    <Container>
      <SectionHeader
        eyebrow="All platforms"
        title="Pick your build."
        description="Native binaries for every supported platform and architecture."
      />

      <div class="mt-12">
        <Tabs.Root value={activeTab} class="w-full">
          <Tabs.List
            class="glass-card grid w-full grid-cols-3 rounded-xl p-1 sm:max-w-md"
          >
            {#each platforms as p}
              {@const Icon = p.icon}
              <Tabs.Trigger
                value={p.id}
                class="flex items-center justify-center gap-2 rounded-lg text-sm font-medium data-[state=active]:bg-background data-[state=active]:shadow-craft-sm"
              >
                <Icon class="size-4" />
                {p.title}
              </Tabs.Trigger>
            {/each}
          </Tabs.List>

          {#each platforms as p}
            {@const Icon = p.icon}
            {@const guide = installSteps[p.id]}
            {@const anchorId =
              p.id === "macOS"
                ? "macos-first-launch"
                : `install-${p.id.toLowerCase()}`}
            {@const stab = stabilityCopy[p.stability]}
            <Tabs.Content value={p.id} class="mt-8">
              <Reveal>
                <article
                  class="glass-card relative overflow-hidden rounded-2xl p-8 sm:p-10"
                >
                  <div
                    class="pointer-events-none absolute -right-16 -top-16 size-48 rounded-full bg-primary/5 blur-3xl"
                  ></div>

                  <div
                    class="relative flex flex-col gap-8 sm:flex-row sm:items-start sm:justify-between"
                  >
                    <div>
                      <span
                        class="glass-chip grid size-12 place-items-center rounded-xl text-foreground/70"
                      >
                        <Icon class="size-5" />
                      </span>
                      <div class="mt-6 flex flex-wrap items-center gap-3">
                        <h3 class="text-2xl font-semibold tracking-tight">
                          {p.title}
                        </h3>
                        <span
                          class={cn(
                            "inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-[0.12em] ring-1 ring-inset",
                            stab.chip,
                          )}
                          title={p.stability === "stable"
                            ? "This is the build I use daily."
                            : "Early port. Please file issues on GitHub when something breaks."}
                        >
                          <span class={cn("size-1.5 rounded-full", stab.dot)}
                          ></span>
                          {p.stability === "stable" ? "Stable" : "Beta"}
                        </span>
                      </div>
                      <p class="mt-1.5 text-sm text-muted-foreground">
                        {p.subtitle}
                      </p>
                    </div>

                    <div class="grid w-full gap-3 sm:max-w-xs">
                      {#each platformAssets[p.id] as asset, i}
                        <Button
                          href={asset.link ?? undefined}
                          disabled={!asset.link}
                          variant="dark"
                          class={cn(
                            "w-full justify-between gap-3",
                            !asset.link && "opacity-60",
                          )}
                        >
                          <span>{asset.label}</span>
                          <ArrowDownToLine class="size-4 opacity-70" />
                        </Button>
                      {/each}
                    </div>
                  </div>

                  <!-- Setup steps — same shape for every platform so adding a new
									     OS is a data change, not a layout one. macOS preserves the
									     `#macos-first-launch` anchor that the hero CTA links to. -->
                  <div id={anchorId} class="relative mt-10">
                    <div
                      class="flex flex-wrap items-center justify-between gap-3"
                    >
                      <div class="flex items-center gap-2.5">
                        <span
                          class="grid size-8 place-items-center rounded-lg bg-foreground/5 text-foreground/70 ring-1 ring-foreground/5"
                        >
                          <Info class="size-4" />
                        </span>
                        <h4
                          class="text-sm font-semibold tracking-tight text-foreground"
                        >
                          Install on {p.title}
                        </h4>
                      </div>
                      <span
                        class="font-mono text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground"
                      >
                        {guide.steps.length} steps
                      </span>
                    </div>
                    <p
                      class="mt-3 max-w-2xl text-sm leading-relaxed text-muted-foreground"
                    >
                      {guide.intro}
                    </p>

                    <ol class="relative mt-6 space-y-3">
                      {#each guide.steps as step, idx}
                        <li
                          class="group/step relative flex gap-4 rounded-2xl border border-border-low/50 bg-foreground/1.5 p-4 transition-colors hover:bg-foreground/3 sm:p-5"
                        >
                          <span
                            class="relative z-10 grid size-8 shrink-0 place-items-center rounded-lg bg-foreground text-background font-mono text-[12px] font-semibold tabular-nums shadow-craft-sm"
                          >
                            {idx + 1}
                          </span>
                          <div class="min-w-0 flex-1 space-y-2.5">
                            <div
                              class="text-sm font-semibold tracking-tight text-foreground"
                            >
                              {step.title}
                            </div>
                            <p
                              class="text-sm leading-relaxed text-muted-foreground"
                            >
                              {step.body}
                            </p>
                            {#if step.code}
                              <pre
                                class="overflow-x-auto rounded-lg border border-border-low/60 bg-foreground/4 px-3 py-2.5 font-mono text-xs leading-relaxed text-foreground"><code
                                  >{step.code}</code
                                ></pre>
                            {/if}
                            {#if step.hint}
                              <p
                                class="flex items-start gap-1.5 text-[11px] leading-relaxed text-muted-foreground/80"
                              >
                                <CheckCircle2
                                  class="mt-0.5 size-3 shrink-0 text-primary/70"
                                />
                                <span>{step.hint}</span>
                              </p>
                            {/if}
                          </div>
                        </li>
                      {/each}
                    </ol>

                    <!-- Troubleshooting — surfaces the common Google searches
										     ("Recast is damaged", "AppImage won't launch") so users
										     don't bounce out to file an issue. -->
                    {#if guide.faqs.length}
                      <div class="mt-8">
                        <div class="flex items-center gap-2.5">
                          <span
                            class="grid size-8 place-items-center rounded-lg bg-amber-500/15 text-amber-600 ring-1 ring-amber-500/15 dark:text-amber-400"
                          >
                            <LifeBuoy class="size-4" />
                          </span>
                          <h4
                            class="text-sm font-semibold tracking-tight text-foreground"
                          >
                            If something goes wrong
                          </h4>
                        </div>
                        <div class="mt-4 grid gap-3 sm:grid-cols-2">
                          {#each guide.faqs as faq}
                            <Collapsible.Root
                              class="group/faq rounded-xl border border-border-low/50 bg-foreground/1.5 p-4 transition-colors hover:bg-foreground/3 data-[state=open]:border-border-low/70"
                            >
                              <Collapsible.Trigger
                                class="flex w-full cursor-pointer items-start justify-between gap-3 text-left"
                              >
                                <span
                                  class="text-sm font-medium text-foreground"
                                >
                                  {faq.title}
                                </span>
                                <ChevronDown
                                  class="mt-0.5 size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-data-[state=open]/faq:rotate-180"
                                />
                              </Collapsible.Trigger>
                              <Collapsible.Content>
                                <div class="mt-3 space-y-2.5">
                                  <p
                                    class="text-sm leading-relaxed text-muted-foreground"
                                  >
                                    {faq.body}
                                  </p>
                                  {#if faq.code}
                                    <pre
                                      class="overflow-x-auto rounded-lg border border-border-low/60 bg-foreground/4 px-3 py-2.5 font-mono text-xs leading-relaxed text-foreground"><code
                                        >{faq.code}</code
                                      ></pre>
                                  {/if}
                                </div>
                              </Collapsible.Content>
                            </Collapsible.Root>
                          {/each}
                        </div>
                      </div>
                    {/if}

                    {#if p.id === "macOS"}
                      <div
                        class="mt-6 flex items-start gap-3 rounded-xl border border-amber-500/25 bg-amber-500/4 p-4 text-xs leading-relaxed text-muted-foreground"
                      >
                        <TriangleAlert
                          class="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-400"
                        />
                        <span>
                          <span class="font-semibold text-foreground"
                            >Heads up:</span
                          >
                          until we ship Apple notarization, the quarantine step above
                          is required on the .dmg path, or just install with Homebrew,
                          which clears it for you. Pasting
                          <span class="font-mono text-foreground/85"
                            >"Recast is damaged"</span
                          >
                          into Google brought you here.
                        </span>
                      </div>
                    {/if}
                  </div>
                </article>
              </Reveal>
            </Tabs.Content>
          {/each}
        </Tabs.Root>
      </div>

      <div
        class="glass-card mt-10 flex flex-col items-start gap-3 rounded-2xl p-5 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between sm:p-6"
      >
        <div class="flex items-center gap-2.5">
          <span
            class="glass-chip grid size-8 place-items-center rounded-lg text-foreground/70"
          >
            <ShieldCheck class="size-4" />
          </span>
          <span>
            Source on
            <a
              href="https://github.com/kanakkholwal/recast"
              target="_blank"
              rel="noopener noreferrer"
              class="font-semibold text-foreground transition-colors hover:text-primary"
            >
              GitHub →
            </a>
          </span>
        </div>
        <span class="font-mono text-xs">
          Verify checksums on the
          <a
            href="https://github.com/kanakkholwal/recast/releases/latest"
            target="_blank"
            rel="noopener noreferrer"
            class="font-semibold text-foreground transition-colors hover:text-primary"
          >
            release page →
          </a>
        </span>
      </div>
    </Container>
  </Section>

  <Footer />
</main>
