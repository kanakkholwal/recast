<script lang="ts">
import { Container, HeroBackdrop, MacWindow, Section, SelectionWord } from "$lib/components";
import { autoplayInView, prefersReducedMotion, TextLoop } from "$lib/motion-core";
import { ArrowRight, CloudDownloadIcon, Megaphone } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { fly } from "svelte/transition";
import { backdropUrl, heroStagger, platforms, rise, steps, words } from "./Hero.logic";

// Svelte transitions use WAAPI, which the CSS reduced-motion guard can't
// reach; gate the mount choreography here so a reduced-motion visitor gets
// the hero fully-formed instead of a staggered fly-in. `duration: 0` keeps
// the directive attached (no markup branching) while removing the motion.
const reduced = $derived(prefersReducedMotion());
const riseM = (delay: number) => (reduced ? { duration: 0 } : rise(delay));

// Hero preview asset. Pass the polished demo URL from the parent (single
// source of truth alongside the before/after proof clips). Falls back to
// the static screenshot if no URL is provided, so the hero never breaks
// on a missing prop.
let { previewSrc = "" }: { previewSrc?: string } = $props();
</script>

<Section
  spacing="none"
  class="relative overflow-hidden pt-36 pb-20 md:pt-44 md:pb-28"
>
  <HeroBackdrop src={backdropUrl} parallax />

  <Container class="relative z-10">
    <div class="mx-auto flex max-w-6xl flex-col items-center text-center">
      <!-- Understated changelog link (was a loud lime pill): muted text with a
			     small icon, so it sits under the headline instead of competing. Same
			     rise as the rest of the hero so it lands on the same breath. -->
      <a
        href="/changelog"
        class="group inline-flex items-center gap-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground"
        in:fly={riseM(heroStagger * 0)}
      >
        <Megaphone class="size-3.5" />
        What's new
        <ArrowRight
          class="size-3 transition-transform group-hover:translate-x-0.5"
        />
      </a>

      <h1
        class="text-balance mt-7 text-3xl font-bold leading-[1.02] tracking-tight text-foreground sm:text-6xl md:text-7xl lg:text-[5.25rem]"
        in:fly={riseM(heroStagger * 1)}
      >
        Record once.
        <span
          class="mt-2 flex justify-center font-medium italic text-foreground/40"
        >
          <span class="whitespace-nowrap">Ship a&nbsp;</span>
          <!-- The rotating output is framed as a selected/editable object:
					     the box tracks TextLoop's width tween and its handles sit
					     outside TextLoop's own clip, so nothing gets cropped. -->
          <SelectionWord>
            <span class="inline-grid overflow-hidden">
              <TextLoop class="text-primary" texts={words} interval={3000} />
            </span>
          </SelectionWord>
        </span>
      </h1>

      <p
        class="text-pretty mt-7 max-w-2xl text-sm leading-relaxed text-muted-foreground sm:text-lg md:text-base"
        in:fly={riseM(heroStagger * 2)}
      >
        Smart zoom, cursor smoothing, and silence cuts happen while you record.
        By the time you stop, the demo is mostly done.
      </p>

      <!-- Record → Auto-polish → Share -->
      <div
        class="mt-8 flex flex-wrap items-center justify-center gap-2 text-xs font-semibold text-muted-foreground"
        in:fly={riseM(heroStagger * 3)}
      >
        {#each steps as step, i}
          {@const Icon = step.icon}
          <span
            class="glass-chip flex items-center gap-1.5 rounded-full px-3 py-1.5 whitespace-nowrap"
          >
            <Icon class="size-3.5 text-primary" />
            {step.label}
          </span>
          {#if i < steps.length - 1}
            <ArrowRight class="size-3.5 text-muted-foreground/40" />
          {/if}
        {/each}
      </div>

      <div
        class="mt-9 flex flex-col items-center gap-3 sm:flex-row sm:gap-4"
        in:fly={riseM(heroStagger * 4)}
      >
        <Button href="/download" variant="dark" size="xl" class="gap-2.5">
          <CloudDownloadIcon class="size-4" />
          Download for Desktop
        </Button>
        <!-- <Button href="#proof" variant="outline" size="lg" class="group/cta gap-2">
					Watch it work
					<ArrowRight class="size-4 transition-transform group-hover/cta:translate-x-0.5" />
				</Button> -->
      </div>

      <div
        class="mt-8 flex items-center gap-2 text-xs font-medium tracking-[0.16em] text-muted-foreground/80"
        in:fly={riseM(heroStagger * 5)}
      >
        <span class="relative flex size-1.5">
          <span
            class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-70"
          ></span>
          <span class="relative inline-flex size-1.5 rounded-full bg-primary"
          ></span>
        </span>
        Free forever · No sign-up
        <span
          class="mx-2 hidden h-1 w-1 rounded-full bg-muted-foreground/40 sm:inline-block"
        ></span>
        <span class="hidden items-center gap-2 sm:inline-flex">
          {#each platforms as p, i}
            <span>{p}</span>
            {#if i < platforms.length - 1}
              <span class="text-muted-foreground/40">·</span>
            {/if}
          {/each}
        </span>
      </div>
    </div>

    <figure class="relative z-10 mx-auto mt-20 max-w-6xl" in:fly={riseM(heroStagger * 6)}>
      <!-- Product floats over the exposed photo band, glass chrome frosting
			     the scene behind it (the dock.cool move). -->
      <MacWindow
        url="recast.li"
        title="Untitled recording"
        class="shadow-craft-xl ring-1 ring-foreground/5"
      >
        <div class="bg-linear-to-b from-muted/10 to-background p-1.5 sm:p-2">
          {#if previewSrc}
            <!-- Polished demo loop. Always silent (same proof framing as
						     the before/after pair below the fold; sound would fight
						     the hero copy and the TextLoop animation).
						     `aspect-video` reserves the 16:9 box so the preview never
						     shifts layout when the clip's metadata loads. -->
            <!-- svelte-ignore a11y_media_has_caption -->
            <video
              use:autoplayInView
              src={previewSrc}
              poster="/product_preview_hero.webp"
              autoplay
              loop
              muted
              playsinline
              preload="metadata"
              class="block aspect-video w-full rounded-xl object-cover ring-1 ring-border-low"
            ></video>
          {:else}
            <img
              src="/product_preview_hero.webp"
              alt="Recast app preview"
              width="1920"
              height="1080"
              loading="eager"
              decoding="async"
              class="block aspect-video w-full rounded-xl object-cover ring-1 ring-border-low"
            />
          {/if}
        </div>
      </MacWindow>

      <div
        class="glass-chip absolute -bottom-4 left-4 z-20 hidden items-center gap-2.5 rounded-xl px-3.5 py-2 shadow-craft-floating sm:flex md:-bottom-5 md:left-8"
        in:fly={riseM(heroStagger * 7)}
      >
        <span class="relative flex size-2">
          <span
            class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/50"
          ></span>
          <span class="relative inline-flex size-2 rounded-full bg-primary"
          ></span>
        </span>
        <span class="text-xs font-semibold text-foreground"
          >Recording · 00:42</span
        >
      </div>

      <div
        class="glass-chip absolute -top-4 right-4 z-20 hidden items-center gap-2 rounded-xl px-3.5 py-2 shadow-craft-floating sm:flex md:-top-5 md:right-8"
        in:fly={riseM(heroStagger * 8)}
      >
        <span class="text-xs font-semibold text-foreground"
          >Cursor smoothed</span
        >
        <span
          class="rounded-md bg-primary/10 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-primary"
        >
          Auto
        </span>
      </div>

      <figcaption
        class="mt-7 text-center text-[12.5px] leading-relaxed text-muted-foreground sm:mt-9"
      >
        One raw take, auto-polished into a demo worth sending.
      </figcaption>
    </figure>
  </Container>
</Section>
