<script lang="ts">
import { authClient } from "$lib/auth/client";
import { Container, HeroSteps, Section, SelectionWord } from "$lib/components";
import { prefersReducedMotion, TextLoop } from "$lib/motion-core";
import { ArrowRight, ArrowUpRight, CloudDownloadIcon } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { fly } from "svelte/transition";
import { heroStagger, platforms, rise, steps, words } from "./Hero.logic";

// The desktop app needs no account; Cloud sharing does. The secondary CTA
// carries that second door, and points at the dashboard once you're in so a
// signed-in visitor isn't asked to sign up again.
const session = authClient.useSession();
const signedIn = $derived(Boolean($session.data?.user));

// Svelte transitions use WAAPI, which the CSS reduced-motion guard can't
// reach; gate the mount choreography here so a reduced-motion visitor gets
// the hero fully-formed instead of a staggered fly-in. `duration: 0` keeps
// the directive attached (no markup branching) while removing the motion.
const reduced = $derived(prefersReducedMotion());
const riseM = (delay: number) => (reduced ? { duration: 0 } : rise(delay));

// Shared take for all three shelf tabs until each step has its own clip.
let { previewSrc = "" }: { previewSrc?: string } = $props();
</script>

<Section spacing="none" class="relative">
  <div class="relative pt-32 pb-14 md:pt-40 md:pb-16">
    <Container class="relative z-10">
      <div class="mx-auto flex max-w-4xl flex-col items-center text-center">
        <!-- Split announcement pill: the claim on the left, the way in on the
             right, one hairline between them. -->
        <a
          href="/changelog"
          class="pill group inline-flex items-center overflow-hidden text-body-sm"
          in:fly={riseM(heroStagger * 0)}
        >
          <span class="py-1.5 pl-4 pr-3 font-medium text-foreground">
            What's new in Recast
          </span>
          <span
            class="inline-flex items-center gap-1 self-stretch border-l border-border-low py-1.5 pl-3 pr-4 text-muted-foreground transition-colors group-hover:text-foreground"
          >
            Read more
            <ArrowUpRight class="size-3.5" />
          </span>
        </a>

        <h1
          class="text-balance mt-8 text-heading-lg leading-[1.05] sm:text-display lg:text-display-lg"
          in:fly={riseM(heroStagger * 1)}
        >
          Record once.
          <span class="mt-2 flex justify-center italic text-muted-foreground">
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
          class="text-pretty mt-7 max-w-2xl text-body text-muted-foreground sm:text-body-lg"
          in:fly={riseM(heroStagger * 2)}
        >
          Smart zoom, cursor smoothing, and silence cuts happen while you record.
          By the time you stop, the demo is mostly done.
        </p>

        <div
          class="mt-9 flex flex-col items-center gap-3 sm:flex-row"
          in:fly={riseM(heroStagger * 3)}
        >
          <Button href="/download" variant="dark" size="lg" class="gap-2">
            <CloudDownloadIcon class="size-4" />
            Download for Desktop
          </Button>
          <Button
            href={signedIn ? "/dashboard" : "/signup"}
            variant="outline"
            size="lg"
            class="group/cta gap-2"
          >
            {signedIn ? "Go to dashboard" : "Share your first demo"}
            <ArrowRight
              class="size-4 transition-transform group-hover/cta:translate-x-0.5"
            />
          </Button>
        </div>

        <div
          class="mt-8 flex flex-wrap items-center justify-center gap-2 text-caption text-muted-foreground"
          in:fly={riseM(heroStagger * 4)}
        >
          Free forever · No account needed to record
          <span class="hidden items-center gap-2 sm:inline-flex">
            <span class="text-border-strong">·</span>
            {#each platforms as p, i}
              <span>{p}</span>
              {#if i < platforms.length - 1}
                <span class="text-border-strong">·</span>
              {/if}
            {/each}
          </span>
        </div>
      </div>
    </Container>
  </div>

  <!-- Full-bleed paper band. The shelf hangs out of the white canvas above it,
       so this must sit outside the Container. -->
  <HeroSteps {steps} fallbackSrc={previewSrc} />
</Section>
