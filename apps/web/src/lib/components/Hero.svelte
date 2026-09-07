<script lang="ts">
import { ArrowRight, ArrowUpRight, CloudDownloadIcon } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { fly } from "svelte/transition";
import { authClient } from "$lib/auth/client";
import { Container, HeroSteps, Section, SelectionWord } from "$lib/components";
import { prefersReducedMotion, TextLoop } from "$lib/motion-core";
import { heroStagger, platforms, rise, steps, words } from "./Hero.logic";

const session = authClient.useSession();
const signedIn = $derived(Boolean($session.data?.user));

const reduced = $derived(prefersReducedMotion());
const riseM = (delay: number) => (reduced ? { duration: 0 } : rise(delay));

// Shared take for all three shelf tabs until each step has its own clip.
let { previewSrc = "" }: { previewSrc?: string } = $props();
</script>

<Section spacing="none" class="relative">
  <div class="relative pt-28 pb-14 md:pt-32 md:pb-16">
    <Container class="relative z-10">
      <div class="mx-auto flex max-w-3xl flex-col items-center text-center">
       
        <a
          href="/changelog"
          class="pill group inline-flex items-center h-7 overflow-hidden text-xs leading-none"
          in:fly={riseM(heroStagger * 0)}
        >
          <span class="py-3 pl-4 pr-3 font-medium text-foreground whitespace-nowrap">
            What's new in Recast
          </span>
          <span
            class="inline-flex items-center gap-1 self-stretch whitespace-nowrap border-l border-border-low py-2 pl-3 pr-4 text-muted-foreground transition-colors group-hover:text-foreground"
          >
            Read more
            <ArrowUpRight class="size-3.5" />
          </span>
        </a>

        <h1
          class="text-balance mt-7 text-heading-lg leading-[1.05] sm:text-heading-lg md:text-display font-display font-semibold"
          in:fly={riseM(heroStagger * 1)}
        >
          Record once.
          <span class="mt-1 flex justify-center italic text-muted-foreground">
            <span class="whitespace-nowrap">Ship a&nbsp;</span>
             <SelectionWord>
              <span class="inline-grid overflow-hidden">
                <TextLoop class="text-primary" texts={words} interval={3000} />
              </span>
            </SelectionWord>
          </span>
        </h1>

        <p
          class="text-pretty mt-6 max-w-xl text-body text-muted-foreground sm:text-body-lg"
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
          class="mt-7 flex flex-wrap items-center justify-center gap-2 text-caption text-muted-foreground"
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

  <HeroSteps {steps} fallbackSrc={previewSrc} />
</Section>
