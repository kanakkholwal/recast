<script lang="ts">
  import { config } from "$constants/app";
  import {
    KIND_META,
    RELEASES,
    type ChangeKind,
    type ChangelogRelease,
  } from "$constants/changelog";
  import { whatsNew } from "$lib/stores/whats-new.svelte";
  import { Github, Sparkles } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  const ORDER: ChangeKind[] = ["added", "changed", "fixed", "deprecated"];

  function groupChanges(release: ChangelogRelease) {
    const map = new Map<ChangeKind, string[]>();
    for (const c of release.changes) {
      if (!map.has(c.kind)) map.set(c.kind, []);
      map.get(c.kind)!.push(c.summary);
    }
    return ORDER.filter((k) => map.has(k)).map(
      (k) => [k, map.get(k)!] as const,
    );
  }

  // Visiting the full changelog page also counts as having seen the latest version.
  onMount(() => {
    whatsNew.markSeen();
  });
</script>

<div class="h-full overflow-y-auto scrollbar-transparent no-scrollbar">
  <div class="mx-auto flex max-w-3xl flex-col gap-8 px-6 py-10">
    <header
      in:fly={{ y: 12, duration: 320, easing: cubicOut }}
      class="flex flex-col gap-3"
    >
      <span
        class="inline-flex w-fit items-center gap-1.5 rounded-full border border-border/50 bg-card/60 px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.15em] text-muted-foreground/80 backdrop-blur"
      >
        <Sparkles class="size-3 text-primary" />
        Changelog
      </span>
      <h1
        class="text-balance text-[28px] font-semibold leading-tight tracking-tight text-foreground md:text-[32px]"
      >
        <span
          class="bg-linear-to-r from-foreground to-foreground/55 bg-clip-text text-transparent"
        >
          Everything new in {config.appName}.
        </span>
      </h1>
      <p class="text-[12.5px] leading-relaxed text-muted-foreground">
        Per-release notes for features, refinements, and fixes. The current
        build is
        <span class="font-mono text-foreground/80">v{config.appVersion}</span>.
      </p>
      <div class="flex flex-wrap gap-2 pt-1">
        <Button
          href={`${config.github}/releases`}
          target="_blank"
          variant="outline"
          size="sm"
          class="h-8 gap-1.5"
        >
          <Github class="size-3.5" />
          <span class="text-[11.5px]">Releases on GitHub</span>
        </Button>
      </div>
    </header>

    <div
      in:fly={{ y: 12, duration: 320, delay: 80, easing: cubicOut }}
      class="flex flex-col gap-10"
    >
      {#each RELEASES as release, i (release.version)}
        {@const isLatest = i === 0}
        <section
          in:fade={{ duration: 220, delay: 120 + i * 40 }}
          class="relative flex flex-col gap-4"
        >
          <div
            class="flex flex-col gap-1.5 border-l-2 pl-4 {isLatest
              ? 'border-primary'
              : 'border-border/60'}"
          >
            <div
              class="flex flex-wrap items-center gap-2 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/80"
            >
              <span class="font-mono normal-case tracking-normal text-foreground">
                v{release.version}
              </span>
              <span class="text-muted-foreground/40">·</span>
              <span class="font-medium normal-case tracking-normal">
                {release.date}
              </span>
              {#if isLatest}
                <span
                  class="rounded-full bg-primary/15 px-2 py-0.5 text-[9px] font-semibold uppercase tracking-[0.12em] text-primary"
                >
                  Latest
                </span>
              {/if}
            </div>
            {#if release.title}
              <h2
                class="text-[18px] font-semibold leading-tight tracking-tight text-foreground"
              >
                {release.title}
              </h2>
            {/if}
          </div>

          {#if release.highlights?.length}
            <ul class="flex flex-col gap-2">
              {#each release.highlights as h (h)}
                <li
                  class="flex items-start gap-2 rounded-lg border border-border/50 bg-card/40 px-3 py-2 text-[12.5px] leading-relaxed text-foreground"
                >
                  <Sparkles class="mt-0.5 size-3.5 shrink-0 text-primary" />
                  <span>{h}</span>
                </li>
              {/each}
            </ul>
          {/if}

          <div class="flex flex-col gap-5">
            {#each groupChanges(release) as [kind, items] (kind)}
              {@const meta = KIND_META[kind]}
              {@const Icon = meta.icon}
              <div class="flex flex-col gap-1.5">
                <div class="flex items-center gap-1.5">
                  <Icon class={`size-3.5 ${meta.tone}`} />
                  <span
                    class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/80"
                  >
                    {meta.label}
                  </span>
                </div>
                <ul class="flex flex-col gap-1 pl-1">
                  {#each items as it (it)}
                    <li
                      class="flex items-start gap-2 text-[12.5px] leading-relaxed text-foreground/90"
                    >
                      <span
                        class="mt-1.5 size-1 shrink-0 rounded-full bg-foreground/30"
                        aria-hidden="true"
                      ></span>
                      <span>{it}</span>
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  </div>
</div>
