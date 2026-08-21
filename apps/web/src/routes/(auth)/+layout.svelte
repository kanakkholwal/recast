<script lang="ts">
import { steps } from "$lib/components/Hero.logic";
import Logo from "$lib/logo.svelte";
import { ArrowLeft } from "@recast/icons";
import { Button } from "@recast/ui/button";

let { children } = $props();

const glyph = {
	tangerine: "text-tag-tangerine",
	lavender: "text-tag-lavender",
	green: "text-tag-green",
} as const;

const facts = ["Free forever", "Works offline", "Open source"];
const year = new Date().getFullYear();
</script>

<div class="min-h-screen text-foreground lg:grid lg:grid-cols-12">
  <div
    class="flex min-h-screen flex-col lg:col-span-7 lg:border-r lg:border-border-low"
  >
    <header
      class="flex items-center justify-between gap-4 border-b border-border-low px-6 py-4 sm:px-10"
    >
      <a
        href="/"
        class="inline-flex items-center gap-2.5"
        aria-label="Recast home"
      >
        <span
          class="grid size-8 place-items-center rounded-lg bg-foreground p-1 text-background"
        >
          <Logo size="20" color="transparent" fill="currentColor" />
        </span>
        <span class="font-display text-lg font-semibold text-foreground"
          >Recast</span
        >
      </a>
      <a
        href="/"
        class="inline-flex items-center gap-1.5 text-body-sm text-muted-foreground underline-offset-4 transition-colors hover:text-foreground hover:underline"
      >
        <ArrowLeft class="size-3.5" />
        Back to site
      </a>
    </header>

    <main class="flex flex-1 items-center px-6 py-6 sm:px-10">
      <div class="w-full max-w-md">
        {@render children()}
      </div>
    </main>
  </div>

  <aside
    class="hidden bg-paper lg:col-span-5 lg:flex lg:flex-col lg:justify-between lg:p-10"
  >
    <div>
      <p class="max-w-xs font-display font-medium text-heading text-foreground">
        Record, polish, share. In one pass.
      </p>

      <ul class="mt-10 divide-y divide-border-low border-y border-border-low">
        {#each steps as step (step.id)}
          {@const Icon = step.icon}
          <li class="flex gap-4 py-5">
            <Icon
              class="mt-0.5 size-5 shrink-0 [fill-opacity:0.2] {glyph[
                step.accent
              ]}"
              fill="currentColor"
            />
            <div class="min-w-0">
              <h2 class="font-display text-body font-medium text-foreground">
                {step.label}
              </h2>
              <p class="mt-1 text-body-sm text-muted-foreground">
                {step.caption}
              </p>
            </div>
          </li>
        {/each}
      </ul>
      <div class="mt-5 text-body-sm text-muted-foreground">
        <Button href="/download" variant="dark">
          Download Desktop App
        </Button>
      </div>
    </div>

    <ul
      class="flex flex-wrap items-center divide-x divide-border-low text-body-sm text-muted-foreground"
    >
      {#each facts as fact (fact)}
        <li class="pr-4 not-first:pl-4">{fact}</li>
      {/each}
    </ul>
  </aside>
</div>
