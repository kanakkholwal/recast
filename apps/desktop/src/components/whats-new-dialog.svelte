<script lang="ts">
  import { config } from "$constants/app";
  import {
    KIND_META,
    LATEST_RELEASE,
    type ChangeKind,
  } from "$constants/changelog";
  import { whatsNew } from "$lib/stores/whats-new.svelte";
  import { ArrowRight, Sparkles } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import * as Dialog from "@recast/ui/dialog";

  // Order changes so additions surface first, then changes, then fixes.
  const ORDER: ChangeKind[] = ["added", "changed", "fixed", "deprecated"];

  const grouped = $derived.by(() => {
    const map = new Map<ChangeKind, string[]>();
    for (const c of LATEST_RELEASE.changes) {
      if (!map.has(c.kind)) map.set(c.kind, []);
      map.get(c.kind)!.push(c.summary);
    }
    return ORDER.filter((k) => map.has(k)).map((k) => [k, map.get(k)!] as const);
  });

  function handleOpenChange(v: boolean) {
    if (!v) whatsNew.dismiss();
    else whatsNew.open = true;
  }
</script>

<Dialog.Root open={whatsNew.open} onOpenChange={handleOpenChange}>
  <Dialog.Content
    class="max-w-xl overflow-hidden rounded-xl p-0 ring-1 ring-border"
  >
    <Dialog.Header class="sr-only">
      <Dialog.Title>What's new in Recast {LATEST_RELEASE.version}</Dialog.Title>
      <Dialog.Description>
        {LATEST_RELEASE.title ?? "Latest release notes"}
      </Dialog.Description>
    </Dialog.Header>

    <header
      class="flex items-start gap-3 border-b border-border/50 bg-card/50 px-5 py-4"
    >
      <div
        class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-inset ring-primary/20"
      >
        <Sparkles class="size-4" />
      </div>
      <div class="min-w-0 flex-1">
        <div
          class="flex flex-wrap items-center gap-2 text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/80"
        >
          <span>What's new</span>
          <span class="text-muted-foreground/40">·</span>
          <span class="font-mono normal-case tracking-normal">
            v{LATEST_RELEASE.version}
          </span>
          <span class="text-muted-foreground/40">·</span>
          <span class="font-medium normal-case tracking-normal">
            {LATEST_RELEASE.date}
          </span>
        </div>
        <h2
          class="mt-1 text-balance text-[16px] font-semibold leading-tight tracking-tight text-foreground"
        >
          {LATEST_RELEASE.title ?? `Recast ${LATEST_RELEASE.version}`}
        </h2>
      </div>
    </header>

    <div class="max-h-[55vh] overflow-y-auto scrollbar-transparent px-5 py-4">
      {#if LATEST_RELEASE.highlights?.length}
        <ul class="mb-4 flex flex-col gap-2">
          {#each LATEST_RELEASE.highlights as h (h)}
            <li
              class="flex items-start gap-2 rounded-lg border border-border/50 bg-muted/20 px-3 py-2 text-[12px] leading-relaxed text-foreground"
            >
              <Sparkles class="mt-0.5 size-3.5 shrink-0 text-primary" />
              <span>{h}</span>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="flex flex-col gap-4">
        {#each grouped as [kind, items] (kind)}
          {@const meta = KIND_META[kind]}
          {@const Icon = meta.icon}
          <section class="flex flex-col gap-1.5">
            <div class="flex items-center gap-1.5 px-1">
              <Icon class={`size-3.5 ${meta.tone}`} />
              <span
                class="text-[10px] font-bold uppercase tracking-[0.15em] text-muted-foreground/80"
              >
                {meta.label}
              </span>
            </div>
            <ul class="flex flex-col gap-1">
              {#each items as it (it)}
                <li
                  class="flex items-start gap-2 rounded-md px-1 py-1 text-[12px] leading-relaxed text-foreground/90"
                >
                  <span
                    class="mt-1.5 size-1 shrink-0 rounded-full bg-foreground/30"
                    aria-hidden="true"
                  ></span>
                  <span>{it}</span>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </div>
    </div>

    <footer
      class="flex items-center justify-between gap-2 border-t border-border/50 bg-muted/30 px-3 py-2 text-[11px] text-muted-foreground"
    >
      <span class="px-1">{config.appName} · v{config.appVersion}</span>
      <div class="flex items-center gap-1.5">
        <Button
          variant="ghost"
          size="xs"
          href="/whats-new"
          onclick={() => whatsNew.dismiss()}
        >
          Full changelog
          <ArrowRight class="ml-1 size-3" />
        </Button>
        <Button size="xs" onclick={() => whatsNew.dismiss()}>Got it</Button>
      </div>
    </footer>
  </Dialog.Content>
</Dialog.Root>
