<script lang="ts">
  import { goto } from "$app/navigation";
  import { config } from "$constants/app";
  import { LATEST_RELEASE } from "$constants/changelog";
  import { updater } from "$lib/stores/updater.svelte";
  import { whatsNew } from "$lib/stores/whats-new.svelte";
  import {
    ArrowRight,
    CircleCheck,
    Download,
    RefreshCw,
    Sparkles,
    TriangleAlert,
    X,
  } from "@lucide/svelte";
  import { Button } from "@recast/ui/button";
  import { cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";

  const pct = $derived(Math.round(updater.progress * 100));

  function openChangelog() {
    whatsNew.dismissCard();
    goto("/whats-new");
  }
</script>

<!-- Non-blocking notification stack pinned to the bottom-right corner. Hosts
     the auto-updater card and the "what's new" card; both stay out of the way
     and never trap focus or block the view. -->
<div
  class="pointer-events-none fixed bottom-4 right-4 z-50 flex w-[320px] flex-col gap-2"
>
  {#if updater.visible}
    <div
      class="pointer-events-auto overflow-hidden rounded-xl border border-border bg-card shadow-lg ring-1 ring-black/5"
      transition:fly={{ y: 16, x: 8, duration: 240, easing: cubicOut }}
    >
      <div class="flex items-start gap-3 px-4 py-3">
        <div
          class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-inset ring-primary/20"
        >
          {#if updater.status === "downloading"}
            <RefreshCw class="size-4 animate-spin" />
          {:else if updater.status === "ready"}
            <CircleCheck class="size-4" />
          {:else}
            <TriangleAlert class="size-4 text-destructive" />
          {/if}
        </div>

        <div class="min-w-0 flex-1">
          <p class="text-[12.5px] font-semibold leading-tight text-foreground">
            {#if updater.status === "downloading"}
              Downloading update
            {:else if updater.status === "ready"}
              Update ready to install
            {:else}
              Update failed
            {/if}
          </p>
          <p class="mt-0.5 text-[11.5px] leading-snug text-muted-foreground">
            {#if updater.status === "error"}
              {updater.error ?? "Could not download the latest version."}
            {:else}
              {config.appName}
              {#if updater.version}
                <span class="font-mono">v{updater.version}</span>
              {/if}
              {updater.status === "ready" ? "is ready." : "is downloading…"}
            {/if}
          </p>
        </div>

        <button
          type="button"
          class="-mr-1 -mt-0.5 shrink-0 rounded-md p-1 text-muted-foreground/70 transition-colors hover:bg-foreground/5 hover:text-foreground"
          aria-label="Dismiss"
          onclick={() => updater.dismiss()}
        >
          <X class="size-3.5" />
        </button>
      </div>

      {#if updater.status === "downloading"}
        <div class="px-4 pb-3">
          <div class="h-1 overflow-hidden rounded-full bg-muted">
            <div
              class="h-full rounded-full bg-primary transition-[width] duration-200"
              style="width: {pct}%"
            ></div>
          </div>
          <span class="mt-1 block text-[10px] font-medium text-muted-foreground">
            {pct}%
          </span>
        </div>
      {:else}
        <div
          class="flex items-center justify-end gap-1.5 border-t border-border/50 bg-muted/30 px-3 py-2"
        >
          {#if updater.status === "ready"}
            <Button
              size="xs"
              disabled={updater.installing}
              onclick={() => updater.installAndRelaunch()}
            >
              <Download class="mr-1 size-3" />
              {updater.installing ? "Installing…" : "Restart to update"}
            </Button>
          {:else if updater.status === "error"}
            <Button size="xs" variant="outline" onclick={() => updater.checkNow()}>
              Retry
            </Button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  {#if whatsNew.cardVisible}
    <div
      class="pointer-events-auto overflow-hidden rounded-xl border border-border bg-card shadow-lg ring-1 ring-black/5"
      transition:fly={{ y: 16, x: 8, duration: 240, delay: 80, easing: cubicOut }}
    >
      <div class="flex items-start gap-3 px-4 py-3">
        <div
          class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-inset ring-primary/20"
        >
          <Sparkles class="size-4" />
        </div>
        <div class="min-w-0 flex-1">
          <p class="text-[12.5px] font-semibold leading-tight text-foreground">
            What's new in {config.appName}
            <span class="font-mono font-normal text-muted-foreground">
              v{LATEST_RELEASE.version}
            </span>
          </p>
          <p class="mt-0.5 line-clamp-2 text-[11.5px] leading-snug text-muted-foreground">
            {LATEST_RELEASE.title ?? "See the latest features, refinements, and fixes."}
          </p>
        </div>
        <button
          type="button"
          class="-mr-1 -mt-0.5 shrink-0 rounded-md p-1 text-muted-foreground/70 transition-colors hover:bg-foreground/5 hover:text-foreground"
          aria-label="Dismiss"
          onclick={() => whatsNew.dismissCard()}
        >
          <X class="size-3.5" />
        </button>
      </div>
      <div
        class="flex items-center justify-end border-t border-border/50 bg-muted/30 px-3 py-2"
      >
        <Button size="xs" variant="ghost" onclick={openChangelog}>
          See what's changed
          <ArrowRight class="ml-1 size-3" />
        </Button>
      </div>
    </div>
  {/if}
</div>
