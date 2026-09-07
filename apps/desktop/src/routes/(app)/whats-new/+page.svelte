<script lang="ts">
import { AiWand } from "@recast/icons";
import { GithubBrand } from "@recast/ui/brand-icons";
import { Button } from "@recast/ui/button";
import { Markdown } from "@recast/ui/markdown";
import { onMount } from "svelte";
import StudioPage from "$components/layout/StudioPage.svelte";
import { config } from "$constants/app";
import { groupChanges, KIND_META, RELEASES } from "$constants/changelog";
import { whatsNew } from "$lib/stores/whats-new.svelte";

// Visiting the full changelog page also counts as having seen the latest version.
onMount(() => {
	whatsNew.markSeen();
});
</script>

<StudioPage
  title="What's New"
  subtitle={`Features, refinements and fixes — current build v${config.appVersion}`}
>
  {#snippet actions()}
    <Button
      href={`${config.github}/releases`}
      target="_blank"
      variant="outline"
      size="sm"
      class="gap-1.5"
    >
      <GithubBrand class="size-3.5" />
      <span class="text-[11.5px]">Releases on GitHub</span>
    </Button>
  {/snippet}

  <div class="mx-auto flex w-full max-w-3xl flex-col gap-10">
      {#each RELEASES as release, i (release.version)}
        {@const isLatest = i === 0}
        <section class="relative flex flex-col gap-4">
          <div
            class="flex flex-col gap-1.5 border-l-2 pl-4 {isLatest
              ? 'border-primary'
              : 'border-border/60'}"
          >
            <div
              class="flex flex-wrap items-center gap-2 text-[10.5px] font-medium text-muted-foreground"
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
                  <AiWand class="mt-0.5 size-3.5 shrink-0 text-primary" />
                  <span><Markdown inline source={h} /></span>
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
                    class="text-[10.5px] font-medium text-muted-foreground"
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
                      <span><Markdown inline source={it} /></span>
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}
          </div>
        </section>
      {/each}
  </div>
</StudioPage>
