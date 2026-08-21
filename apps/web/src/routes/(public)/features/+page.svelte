<script lang="ts">
import {
	Container,
	ExportMock,
	Footer,
	IntegrationGrid,
	PolishMock,
	Reveal,
	Section,
	SectionLabel,
	SeoMeta,
} from "$lib/components";
import { TextLoop } from "$lib/motion-core";
import { ArrowRight, Check, Layers, Share2, Sparkles, Star } from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import { cn } from "@recast/ui/utils";
import { gapRows, integrations, pillars, platforms, stabilityChip, supports, verbs } from "./data";

const REQUEST_URL = "https://github.com/kanakkholwal/recast/issues/new";

const pillarGlyph = ["text-tag-tangerine", "text-tag-lavender", "text-tag-green"];
</script>

<SeoMeta
  title="Everything Recast does for you"
  description="Recording profiles, pause and resume, smart auto-zoom, cursor smoothing, silence cuts, on-frame annotations, Drive sharing. The full feature catalog."
  eyebrow="Features"
/>

<main class="text-foreground">
  <section
    class="mx-auto w-full max-w-6xl border-b border-border-low pt-32 md:pt-40"
  >
    <Container class="pb-12">
      <Reveal variant="up">
        <SectionLabel icon={Sparkles} label="Features" />
      </Reveal>
      <Reveal variant="up" delay={60} class="mt-5">
        <h1
          class="max-w-3xl font-semibold font-display text-balance text-heading-lg md:text-display"
        >
          Everything Recast
          <span class="inline-grid overflow-hidden align-bottom">
            <TextLoop class="text-primary" texts={verbs} interval={2800} />
          </span>
        </h1>
      </Reveal>
      <Reveal variant="up" delay={120} class="mt-4">
        <p class="max-w-xl text-pretty text-body-lg text-muted-foreground">
          Auto-polish for the 80% case, a minimal timeline for the moments you
          want to control.
        </p>
      </Reveal>
      <Reveal
        variant="up"
        delay={180}
        class="mt-8 flex flex-wrap items-center gap-3"
      >
        <Button href="/download" variant="dark" class="gap-2"
          >Download free</Button
        >
        <Button href="/pricing" variant="outline">See pricing</Button>
      </Reveal>
    </Container>

    <Container class="border-t border-border-low">
      <ul class="flex flex-wrap items-center divide-x divide-border-low py-4">
        {#each platforms as p (p.label)}
          {@const Icon = p.icon}
          {@const chip = stabilityChip[p.stability]}
          <li
            class="inline-flex items-center gap-2 pr-4 text-body-sm text-muted-foreground not-first:pl-4"
          >
            <Icon class="size-4 shrink-0" />
            {p.label}
            <span
              class={cn(
                "rounded-full px-1.5 py-0.5 text-caption font-medium",
                chip.cls,
              )}
            >
              {chip.label}
            </span>
          </li>
        {/each}
      </ul>
    </Container>
  </section>

  <!-- Three pillars, one hairline grid. -->
  <section class="mx-auto w-full max-w-6xl border-b border-border-low">
    <Container>
      <div class="grid grid-cols-1 gap-px bg-border-low lg:grid-cols-3">
        {#each pillars as pillar, i (pillar.title)}
          {@const Icon = pillar.icon}
          <Reveal
            variant="up"
            delay={i * 80}
            as="article"
            class="flex h-full flex-col bg-background px-6 py-10"
          >
            <Icon
              class={cn("size-5 [fill-opacity:0.2]", pillarGlyph[i])}
              fill="currentColor"
            />
            <h2
              class="mt-5 font-display text-subheading font-medium text-foreground"
            >
              {pillar.title}
            </h2>
            <p class="mt-2 text-pretty text-body-sm text-muted-foreground">
              {pillar.description}
            </p>
            <ul
              class="mt-5 flex flex-wrap items-center gap-x-3 gap-y-1 text-caption text-muted-foreground"
            >
              {#each pillar.tags as tag, ti (tag)}
                <li class="inline-flex items-center gap-3">
                  {#if ti > 0}
                    <span aria-hidden="true" class="text-border-strong">·</span>
                  {/if}
                  {tag}
                </li>
              {/each}
            </ul>
          </Reveal>
        {/each}
      </div>
    </Container>
  </section>

   <Section
    id="integrations"
    class="mx-auto max-w-6xl border-b border-border-low"
    spacing="tight"
  >
    <Container>
      <Reveal variant="up">
        <div class="flex items-center gap-4 border-b border-border-low pb-5">
          <SectionLabel icon={Share2} label="Integrations" accent="green" />
          <Button
            href={REQUEST_URL}
            variant="outline"
            size="sm"
            class="ml-auto shrink-0"
            target="_blank"
          >
            Request one
          </Button>
        </div>
      </Reveal>

      <div class="grid items-center gap-10 py-10 md:grid-cols-12 md:gap-12">
        <div class="md:col-span-5">
          <Reveal variant="up" delay={60}>
            <h2
              class="font-display font-medium text-balance text-heading md:text-heading-lg"
            >
              Share where your team already is
            </h2>
          </Reveal>
          <Reveal variant="up" delay={120} class="mt-4">
            <p class="text-pretty text-body-lg text-muted-foreground">
              Exports go straight to your own account, never through Recast
              servers.
            </p>
          </Reveal>
          <Reveal variant="up" delay={180} class="mt-6">
            <p
              class="flex items-start gap-2 text-body-sm text-muted-foreground"
            >
              <span
                aria-hidden="true"
                class="mt-1.5 size-1.5 shrink-0 rounded-full bg-tag-green"
              ></span>
              <span>
                Google Drive ships today. The rest are next, and the order
                follows what people ask for.
              </span>
            </p>
          </Reveal>
        </div>

        <div class="md:col-span-7">
          <IntegrationGrid items={integrations} />
        </div>
      </div>
    </Container>
  </Section>

  <!-- Free here, paid elsewhere. Conservative claims, public tiers only. -->
  <Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
    <Container>
      <Reveal variant="up">
        <div class="flex items-center gap-4 border-b border-border-low pb-5">
          <SectionLabel icon={Star} label="Side by side" />
        </div>
      </Reveal>

      <div class="max-w-lg py-10">
        <Reveal variant="up" delay={60}>
          <h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
            Free here. Paid in the others.
          </h2>
        </Reveal>
        <Reveal variant="up" delay={120} class="mt-4">
          <p class="text-pretty text-body-lg text-muted-foreground">
            Most of what the free app ships is paywalled or missing elsewhere.
          </p>
        </Reveal>
      </div>

      <Reveal variant="up" delay={160} class="overflow-x-auto">
        <div class="min-w-160">
          <div
            class="grid grid-cols-[1.6fr_1fr_1fr_1fr] border-y border-border-low py-3 text-caption font-medium text-muted-foreground"
          >
            <div class="pr-4">Feature</div>
            <div class="px-4 text-center text-foreground">Recast</div>
            <div class="px-4 text-center">Loom</div>
            <div class="px-4 text-center">Cap</div>
          </div>
          {#each gapRows as row (row.feature)}
            <div
              class="grid grid-cols-[1.6fr_1fr_1fr_1fr] border-b border-border-low"
            >
              <div class="py-3.5 pr-4 text-body-sm text-foreground">
                {row.feature}
              </div>
              <div
                class="flex items-center justify-center gap-1.5 px-4 py-3.5 text-center"
              >
                <Check class="size-3.5 shrink-0 text-tag-green" />
                <span class="text-caption font-medium text-foreground"
                  >{row.recast}</span
                >
              </div>
              <div
                class="px-4 py-3.5 text-center text-caption text-muted-foreground"
              >
                {row.loom}
              </div>
              <div
                class="px-4 py-3.5 text-center text-caption text-muted-foreground"
              >
                {row.cap}
              </div>
            </div>
          {/each}
        </div>
      </Reveal>

      <Reveal variant="up" delay={200} class="mt-5">
        <p class="text-caption text-muted-foreground">
          Based on each product's publicly documented tiers. Got a correction?
          Open an issue on
          <a
            href="https://github.com/kanakkholwal/recast"
            class="text-foreground underline-offset-4 hover:underline"
          >
            GitHub
          </a>.
        </p>
      </Reveal>
    </Container>
  </Section>


  <section class="mx-auto w-full max-w-6xl border-b border-border-low">
    <Container>
      <div class="grid grid-cols-1 gap-px bg-border-low lg:grid-cols-2">
        <Reveal
          variant="up"
          as="article"
          class="flex flex-col bg-background px-6 py-10"
        >
          <div class="rounded-xl border border-border-low bg-paper">
            <PolishMock />
          </div>
          <h2
            class="mt-8 font-display text-subheading font-medium text-foreground"
          >
            The editing happens while you record
          </h2>
          <p
            class="mt-2 max-w-md text-pretty text-body-sm text-muted-foreground"
          >
            Smart zoom, silence cuts and cursor smoothing land as the take does.
            The timeline is there when you want it.
          </p>
          <Button
            href="/#polish"
            variant="outline"
            size="sm"
            class="mt-6 self-start"
          >
            See how it works
          </Button>
        </Reveal>

        <Reveal
          variant="up"
          delay={80}
          as="article"
          class="flex flex-col bg-background px-6 py-10"
        >
          <div class="rounded-xl border border-border-low bg-paper">
            <ExportMock />
          </div>
          <h2
            class="mt-8 font-display text-subheading font-medium text-foreground"
          >
            Ship a link, not a file
          </h2>
          <p
            class="mt-2 max-w-md text-pretty text-body-sm text-muted-foreground"
          >
            Export, upload and share in one pass. The file goes to your own
            account, never through Recast servers.
          </p>
          <Button
            href="/#share"
            variant="outline"
            size="sm"
            class="mt-6 self-start"
          >
            See sharing
          </Button>
        </Reveal>
      </div>
    </Container>
  </section>

  <!-- Full catalog -->
  <Section class="mx-auto max-w-6xl border-b border-border-low" spacing="tight">
    <Container>
      <Reveal variant="up">
        <div class="flex items-center gap-4 border-b border-border-low pb-5">
          <SectionLabel icon={Layers} label="Built in" accent="lavender" />
          <Button
            href="/changelog"
            variant="outline"
            size="sm"
            class="ml-auto shrink-0"
          >
            Changelog
          </Button>
        </div>
      </Reveal>

      <div class="max-w-lg py-10">
        <Reveal variant="up" delay={60}>
          <h2 class="font-display font-medium text-balance text-heading md:text-heading-lg">
            The full catalog
          </h2>
        </Reveal>
        <Reveal variant="up" delay={120} class="mt-4">
          <p class="text-pretty text-body-lg text-muted-foreground">
            Every affordance worth naming, all shipping in the free desktop app.
          </p>
        </Reveal>
      </div>

      <div
        class="grid grid-cols-1 gap-px border-y border-border-low bg-border-low sm:grid-cols-2 lg:grid-cols-4"
      >
        {#each supports as item, i (item.title)}
          {@const Icon = item.icon}
          <Reveal
            variant="up"
            delay={Math.min(i, 8) * 40}
            as="article"
            class="group/card flex h-full flex-col bg-background px-6 py-8"
          >
            <div class="flex items-center justify-between gap-3">
              <Icon
                class="size-5 text-tag-tangerine [fill-opacity:0.2]"
                fill="currentColor"
              />
              <span class="text-caption text-border-strong">{item.tag}</span>
            </div>
            <h3 class="mt-4 font-display text-body font-medium text-foreground">
              {item.title}
            </h3>
            <p class="mt-2 text-body-sm text-muted-foreground">
              {item.description}
            </p>
          </Reveal>
        {/each}
      </div>
    </Container>
  </Section>

  <Section
    id="cta"
    class="mx-auto max-w-6xl border-b border-border-low"
    spacing="tight"
  >
    <Container>
      <div class="grid gap-10 md:grid-cols-12 md:gap-12">
        <div class="md:col-span-5">
          <Reveal variant="up">
            <h2
              class="font-display font-medium text-balance text-heading md:text-heading-lg"
            >
              Skip the editor. Ship the demo.
            </h2>
          </Reveal>
          <Reveal variant="up" delay={80} class="mt-4">
            <p class="text-pretty text-body-lg text-muted-foreground">
              Free forever, no account. Windows is stable, macOS and Linux are
              in beta.
            </p>
          </Reveal>
          <Reveal variant="up" delay={140} class="mt-6">
            <a
              href="https://github.com/kanakkholwal/recast"
              target="_blank"
              rel="noopener noreferrer"
              class="group/src inline-flex items-center gap-2 text-body-sm text-muted-foreground hover:text-foreground"
            >
              <GithubBrand class="size-4" />
              Read the source
              <ArrowRight
                class="size-3.5 transition-transform group-hover/src:translate-x-0.5"
              />
            </a>
          </Reveal>
        </div>

        <ul
          class="divide-y divide-border-low border-y border-border-low md:col-span-6 md:col-start-7"
        >
          {#each platforms as p, i (p.label)}
            {@const Icon = p.icon}
            <Reveal
              variant="up"
              delay={i * 70}
              as="li"
              class="flex flex-wrap items-center gap-4 py-4"
            >
              <Icon class="size-5 shrink-0 text-foreground" />
              <div class="min-w-0 flex-1">
                <div class="text-body font-medium text-foreground">
                  {p.label}
                </div>
                <div class="text-body-sm text-muted-foreground">{p.note}</div>
              </div>
              <Button
                href={`/download?os=${p.label.toLowerCase()}`}
                variant={p.stability === "stable" ? "dark" : "outline"}
                size="sm"
                class="shrink-0"
              >
                Download
              </Button>
            </Reveal>
          {/each}
        </ul>
      </div>
    </Container>
  </Section>

  <Footer />
</main>
